//! Claude API time is a cumulative session counter, never the wall time of a turn.
use crate::{finops::Usage, model::Run};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct Snapshot {
    at: i64,
    total: u64,
    start: i64,
}

pub fn files(home: &Path) -> Vec<PathBuf> {
    let dir = home.join(".claude/projects");
    let mut files = Vec::new();
    if !dir
        .ancestors()
        .take_while(|p| p.starts_with(home))
        .any(|p| p.is_symlink())
    {
        crate::finops::pi_files(&dir, &mut files, 0);
    }
    files
}

fn snapshots(
    files: &[PathBuf],
    sid: &str,
    workspace: &str,
    budget: &mut u64,
) -> Option<Vec<Snapshot>> {
    uuid::Uuid::parse_str(sid).ok()?;
    let name = format!("{sid}.jsonl");
    let mut matches = files
        .iter()
        .filter(|p| p.file_name().is_some_and(|n| n == name.as_str()));
    let path = matches.next()?;
    if matches.next().is_some() || path.is_symlink() {
        return None;
    }
    let size = path.metadata().ok()?.len();
    if size > 32 * 1024 * 1024 || *budget + size > 128 * 1024 * 1024 {
        return None;
    }
    *budget += size;
    let file = std::fs::File::open(path).ok()?;
    let mut at = None;
    let mut result = Vec::new();
    let mut previous = 0;
    for line in BufReader::new(file).lines() {
        let v: Value = serde_json::from_str(&line.ok()?).ok()?;
        if v["sessionId"].as_str() != Some(sid) {
            continue;
        }
        if v["type"] == "assistant" {
            let cwd = Path::new(v["cwd"].as_str()?);
            if !cwd.is_absolute()
                || !cwd.starts_with(workspace)
                || cwd
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return None;
            }
            at = v["timestamp"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp_millis());
        }
        if v["type"] != "cost-state" {
            continue;
        }
        let total = v["totalAPIDuration"].as_u64()?;
        // A reset or truncated history cannot establish a safe delta.
        if total < previous {
            return None;
        }
        previous = total;
        if let Some(at) = at {
            result.push(Snapshot {
                at,
                total,
                start: v["startTime"].as_i64().unwrap_or(i64::MIN),
            });
        } else if total > 0 {
            return None;
        }
    }
    Some(result)
}

/// Read before starting a resumed CLI. Missing history must not turn its cumulative
/// API counter into this invocation's duration.
pub fn baseline(home: &Path, sid: &str, workspace: &str) -> Option<u64> {
    if sid.is_empty() {
        return Some(0);
    }
    snapshots(&files(home), sid, workspace, &mut 0)?
        .last()
        .map(|s| s.total)
}

/// Enrich existing records only: token, cost and call counts remain untouched.
pub fn enrich(
    r: &Run,
    files: &[PathBuf],
    usage: &mut BTreeMap<String, Usage>,
    budget: &mut u64,
    binding_override: Option<&str>,
) {
    for (binding, session) in &r.sessions {
        if session.runtime != "claude" {
            continue;
        }
        let Some(rows) = snapshots(files, &session.native_id, &r.workspace, budget) else {
            continue;
        };
        let binding = binding_override.unwrap_or(binding);
        let mut previous = 0;
        let mut last_at = None;
        let mut cutoff = i64::MIN;
        for row in rows {
            let delta = row.total - previous;
            previous = row.total;
            // Repeated saves of the same snapshot must not consume a second usage record.
            if last_at == Some(row.at) {
                continue;
            }
            last_at = Some(row.at);
            let candidates: Vec<_> = usage
                .values()
                .filter(|u| {
                    u.runtime == "claude"
                        && u.binding == binding
                        && u.at >= row.start
                        && u.at >= row.at
                        && u.at - row.at <= 10_000
                })
                .map(|u| u.id.clone())
                .collect();
            // Require an unambiguous association to a recorded turn, even if its
            // duration is already present. Never reassign its snapshot to another turn.
            if candidates.len() == 1 {
                let at = usage[&candidates[0]].at;
                // If an earlier recorded turn lacks its snapshot, the counter
                // delta spans multiple turns and cannot be assigned to this one.
                let gap = usage.values().any(|u| {
                    u.runtime == "claude"
                        && u.binding == binding
                        && u.at >= row.start
                        && u.at > cutoff
                        && u.at < at
                });
                let u = usage.get_mut(&candidates[0]).unwrap();
                if !gap && u.llm_ms.is_none() {
                    u.llm_ms = Some(delta);
                }
                cutoff = at;
            } else {
                cutoff = row.at;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{finops::Tracker, model::NativeSession};
    use serde_json::json;

    fn row(sid: &str, cwd: &str, at: i64, total: u64) -> String {
        let timestamp = chrono::DateTime::from_timestamp_millis(at)
            .unwrap()
            .to_rfc3339();
        format!(
            "{}\n{}\n",
            json!({"type":"assistant","sessionId":sid,"cwd":cwd,"timestamp":timestamp}),
            json!({"type":"cost-state","sessionId":sid,"totalAPIDuration":total})
        )
    }
    fn usage(at: i64, id: &str) -> Usage {
        let mut t = Tracker::new("claude", "task", "author", "model", 1);
        let mut u = t.observe(&json!({"type":"result","total_cost_usd":0.5,"usage":{"input_tokens":10,"output_tokens":5}}), "").unwrap();
        u.id = id.into();
        u.at = at;
        u
    }
    #[test]
    fn live_claude_uses_api_counter_delta_and_preserves_missing_and_zero() {
        let mut t = Tracker::new("claude", "task", "author", "model", 1);
        t.claude_api_baseline = Some(100);
        let v = json!({"type":"result","duration_api_ms":350,"duration_ms":9000,"usage":{"input_tokens":10,"output_tokens":5}});
        assert_eq!(t.observe(&v, "").unwrap().llm_ms, Some(250));
        assert!(t.observe(&v, "").is_none());
        for (baseline, total, expected) in [
            (Some(100), json!(0), Some(0)),
            (None, json!(350), None),
            (Some(100), json!(90), None),
            (Some(0), Value::Null, None),
        ] {
            let mut t = Tracker::new("claude", "task", "author", "model", 1);
            t.claude_api_baseline = baseline;
            let mut v = v.clone();
            v["duration_api_ms"] = total;
            assert_eq!(t.observe(&v, "").unwrap().llm_ms, expected);
        }
        let mut t = Tracker::new("codex", "task", "author", "model", 1);
        assert_eq!(t.observe(&json!({"type":"turn.completed","duration_ms":9000,"usage":{"input_tokens":10,"output_tokens":5}}), "").unwrap().llm_ms, None);
    }
    #[tokio::test]
    async fn history_enriches_existing_records_once_with_resumed_deltas_and_ownership() {
        let d = tempfile::tempdir().unwrap();
        let app = crate::tests::app(d.path());
        let mut r = crate::engine::create(
            &app,
            1,
            crate::presets::all().remove(0),
            "test".into(),
            None,
        )
        .await
        .unwrap();
        let sid = crate::model::id();
        r.sessions.insert(
            "author".into(),
            NativeSession {
                provider: String::new(), base_url: String::new(),
runtime: "claude".into(),
                native_id: sid.clone(),
                turns: 2,
                model: "model".into(),
            },
        );
        app.store.save_run(1, &r).unwrap();
        let home = app.data.join("users/1");
        let dir = home.join(".claude/projects/project");
        std::fs::create_dir_all(&dir).unwrap();
        let log = row(&sid, &format!("{}/source", r.workspace), 100_000, 100)
            + &row(&sid, &r.workspace, 100_000, 100)
            + &row(&sid, &r.workspace, 200_000, 350);
        let path = dir.join(format!("{sid}.jsonl"));
        std::fs::write(&path, &log).unwrap();
        for u in [usage(100_050, "first"), usage(200_050, "second")] {
            app.store
                .event(
                    1,
                    &r.id,
                    "usage",
                    "task",
                    &serde_json::to_string(&u).unwrap(),
                )
                .unwrap();
        }
        for _ in 0..2 {
            let report = crate::finops::report(&app, 1).unwrap();
            let m = &report["summary"]["metrics"];
            assert_eq!(m["llm_ms"], 350);
            assert_eq!(m["records"], 2);
            assert_eq!(m["total"], 30);
            assert_eq!(m["usd"], 1.0);
            assert!(
                crate::finops::report(&app, 2).unwrap()["summary"]["metrics"]["llm_ms"].is_null()
            );
        }
        assert_eq!(baseline(&home, &sid, &r.workspace), Some(350));
        let mut records = BTreeMap::from([
            ("first".into(), usage(100_050, "first")),
            ("second".into(), usage(200_050, "second")),
        ]);
        records.get_mut("first").unwrap().llm_ms = Some(100);
        enrich(&r, &files(&home), &mut records, &mut 0, None);
        assert_eq!(records["second"].llm_ms, Some(250));
        enrich(&r, &files(&home), &mut records, &mut 0, None);
        assert_eq!(records["second"].llm_ms, Some(250));
        // Unmatched earlier snapshots still advance the counter baseline.
        records.remove("first");
        records.get_mut("second").unwrap().llm_ms = None;
        enrich(&r, &files(&home), &mut records, &mut 0, None);
        assert_eq!(records["second"].llm_ms, Some(250));
        // A missing first snapshot must not assign the cumulative total to turn two.
        std::fs::write(&path, row(&sid, &r.workspace, 200_000, 350)).unwrap();
        records.insert("first".into(), usage(100_050, "first"));
        records.get_mut("second").unwrap().llm_ms = None;
        enrich(&r, &files(&home), &mut records, &mut 0, None);
        assert_eq!(records["second"].llm_ms, None);
        // Retired manager sessions use the manager binding, not their storage key.
        std::fs::write(&path, &log).unwrap();
        for u in records.values_mut() {
            u.binding = crate::manager::BINDING.into();
        }
        enrich(
            &r,
            &files(&home),
            &mut records,
            &mut 0,
            Some(crate::manager::BINDING),
        );
        assert_eq!(records["first"].llm_ms, Some(100));
        assert_eq!(records["second"].llm_ms, Some(250));
        for u in records.values_mut() {
            u.binding = "author".into();
        }
        // Wrong workspace, malformed history and counter resets never yield a duration.
        for log in [
            row(&sid, "/other", 200_000, 350),
            row(&sid, &format!("{}/../other", r.workspace), 200_000, 350),
            format!("{log}invalid\n"),
            row(&sid, &r.workspace, 100_000, 350) + &row(&sid, &r.workspace, 200_000, 100),
        ] {
            std::fs::write(&path, log).unwrap();
            records.get_mut("second").unwrap().llm_ms = None;
            enrich(&r, &files(&home), &mut records, &mut 0, None);
            assert_eq!(records["second"].llm_ms, None);
        }
        // Symlinked session files are excluded.
        std::fs::remove_file(&path).unwrap();
        let outside = d.path().join("outside.jsonl");
        std::fs::write(&outside, row(&sid, &r.workspace, 200_000, 350)).unwrap();
        std::os::unix::fs::symlink(outside, &path).unwrap();
        assert!(files(&home).is_empty());
    }
}
