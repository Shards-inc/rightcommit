use crate::{
    App,
    auth::{ApiResult, bad},
    model::*,
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

pub fn ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Usage {
    pub id: String,
    pub node: String,
    pub binding: String,
    pub runtime: String,
    pub model: String,
    pub attempt: u32,
    pub at: i64,
    pub input: u64,
    pub output: u64,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub reasoning: Option<u64>,
    pub usd: Option<f64>,
    pub llm_ms: Option<u64>,
    pub calls: Option<u64>,
    pub tools: u64,
    pub source: String,
}
fn n(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or(0)
}
fn finite(v: &Value) -> Option<f64> {
    v.as_f64().filter(|x| x.is_finite() && *x >= 0.)
}
fn pi_usage(
    m: &Value,
    sid: &str,
    node: &str,
    binding: &str,
    attempt: u32,
    source: &str,
) -> Option<Usage> {
    let u = &m["usage"];
    if !u["input"].is_number() || !u["output"].is_number() {
        return None;
    }
    let at = m["timestamp"].as_i64()?;
    let response = m["responseId"].as_str().unwrap_or("");
    let tools = m["content"]
        .as_array()
        .map(|a| a.iter().filter(|c| c["type"] == "toolCall").count() as u64)
        .unwrap_or(0);
    Some(Usage {
        id: crate::store::hash(format!("pi:{sid}:{at}:{response}")),
        node: node.into(),
        binding: binding.into(),
        runtime: "pi".into(),
        model: m["model"].as_str().unwrap_or("unknown").into(),
        attempt,
        at,
        input: n(u, "input") + n(u, "cacheRead") + n(u, "cacheWrite"),
        output: n(u, "output"),
        cache_read: u["cacheRead"].as_u64(),
        cache_write: u["cacheWrite"].as_u64(),
        reasoning: u["reasoning"].as_u64(),
        usd: finite(&u["cost"]["total"]),
        llm_ms: m["durationMs"].as_u64(),
        calls: Some(1),
        tools,
        source: source.into(),
    })
}
pub struct Tracker {
    runtime: String,
    node: String,
    binding: String,
    model: String,
    attempt: u32,
    id: String,
    seen: HashSet<String>,
    tools: HashSet<String>,
    messages: HashSet<String>,
    pub claude_api_baseline: Option<u64>,
}
impl Tracker {
    pub fn new(runtime: &str, node: &str, binding: &str, model: &str, attempt: u32) -> Self {
        Self {
            runtime: runtime.into(),
            node: node.into(),
            binding: binding.into(),
            model: model.into(),
            attempt,
            id: crate::model::id(),
            seen: HashSet::new(),
            tools: HashSet::new(),
            messages: HashSet::new(),
            claude_api_baseline: Some(0),
        }
    }
    pub fn observe(&mut self, v: &Value, sid: &str) -> Option<Usage> {
        let ty = v["type"].as_str().unwrap_or("");
        if ty == "system"
            && v["subtype"] == "init"
            && let Some(model) = v["model"].as_str()
        {
            self.model = model.into()
        }
        if ty == "assistant" {
            if let Some(id) = v["message"]["id"].as_str() {
                self.messages.insert(id.into());
            }
            if let Some(content) = v["message"]["content"].as_array() {
                for c in content {
                    if c["type"] == "tool_use"
                        && let Some(id) = c["id"].as_str()
                    {
                        self.tools.insert(id.into());
                    }
                }
            }
        }
        if ["item.started", "item.completed"].contains(&ty)
            && [
                "command_execution",
                "file_change",
                "mcp_tool_call",
                "web_search",
            ]
            .contains(&v["item"]["type"].as_str().unwrap_or(""))
            && let Some(id) = v["item"]["id"].as_str()
        {
            self.tools.insert(id.into());
        }
        let usage =
            if self.runtime == "pi" && ty == "message_end" && v["message"]["role"] == "assistant" {
                pi_usage(
                    &v["message"],
                    sid,
                    &self.node,
                    &self.binding,
                    self.attempt,
                    "live",
                )
            } else if (self.runtime == "claude" && ty == "result")
                || (self.runtime == "codex" && ty == "turn.completed")
            {
                let u = &v["usage"];
                if !u["input_tokens"].is_number() || !u["output_tokens"].is_number() {
                    return None;
                }
                let claude = self.runtime == "claude";
                Some(Usage {
                    id: format!("{}:summary", self.id),
                    node: self.node.clone(),
                    binding: self.binding.clone(),
                    runtime: self.runtime.clone(),
                    model: self.model.clone(),
                    attempt: self.attempt,
                    at: ms(),
                    input: n(u, "input_tokens")
                        + if claude {
                            n(u, "cache_read_input_tokens") + n(u, "cache_creation_input_tokens")
                        } else {
                            0
                        },
                    output: n(u, "output_tokens"),
                    cache_read: u[if claude {
                        "cache_read_input_tokens"
                    } else {
                        "cached_input_tokens"
                    }]
                    .as_u64(),
                    cache_write: if claude {
                        u["cache_creation_input_tokens"].as_u64()
                    } else {
                        u["cache_write_input_tokens"].as_u64()
                    },
                    reasoning: u["reasoning_output_tokens"]
                        .as_u64()
                        .or_else(|| u["output_tokens_details"]["reasoning_tokens"].as_u64()),
                    usd: if claude {
                        finite(&v["total_cost_usd"])
                    } else {
                        None
                    },
                    llm_ms: if claude {
                        v["duration_api_ms"].as_u64().and_then(|total| {
                            if total == 0 {
                                Some(0)
                            } else {
                                total.checked_sub(self.claude_api_baseline?)
                            }
                        })
                    } else {
                        None
                    },
                    calls: if claude && !self.messages.is_empty() {
                        Some(self.messages.len() as u64)
                    } else {
                        None
                    },
                    tools: 0,
                    source: "live".into(),
                })
            } else {
                None
            };
        usage.filter(|u| self.seen.insert(u.id.clone()))
    }
    pub fn finish(&self, duration: u64, status: &str) -> Value {
        json!({"id":self.id,"node":self.node,"binding":self.binding,"runtime":self.runtime,"model":self.model,"attempt":self.attempt,"duration_ms":duration,"status":status,"tools":if self.runtime=="pi" {None}else{Some(self.tools.len())}})
    }
}
#[derive(Clone, Default, Serialize)]
struct Metrics {
    records: u64,
    input: u64,
    output: u64,
    total: u64,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    reasoning: Option<u64>,
    usd: Option<f64>,
    estimated_usd: Option<f64>,
    estimated_records: u64,
    approximate_records: u64,
    historical_codex_records: u64,
    total_usd: Option<f64>,
    costed_records: u64,
    llm_ms: Option<u64>,
    llm_calls: Option<u64>,
    tools: u64,
    repeated_usd: Option<f64>,
    agent_ms: Option<u64>,
}
fn add_opt<T: std::ops::Add<Output = T> + Copy>(a: &mut Option<T>, b: Option<T>) {
    if let Some(b) = b {
        *a = Some(a.map(|x| x + b).unwrap_or(b));
    }
}
impl Metrics {
    #[cfg(test)]
    fn add(&mut self, u: &Usage) {
        self.add_priced(u, &crate::codex_cost::Catalog::default());
    }
    fn add_priced(&mut self, u: &Usage, prices: &crate::codex_cost::Catalog) {
        let estimate = prices.estimate(u);
        add_opt(&mut self.estimated_usd, estimate);
        add_opt(&mut self.total_usd, u.usd.or(estimate));
        if estimate.is_some() {
            self.estimated_records += 1;
            if u.source != "codex_native_history" || u.cache_write.is_none() {
                self.approximate_records += 1;
            }
        }
        if u.source == "codex_native_history" {
            self.historical_codex_records += 1;
        }
        self.records += 1;
        self.input += u.input;
        self.output += u.output;
        self.total += u.input + u.output;
        add_opt(&mut self.cache_read, u.cache_read);
        add_opt(&mut self.cache_write, u.cache_write);
        add_opt(&mut self.reasoning, u.reasoning);
        add_opt(&mut self.usd, u.usd);
        add_opt(&mut self.llm_ms, u.llm_ms);
        add_opt(&mut self.llm_calls, u.calls);
        self.tools += u.tools;
        if u.usd.is_some() {
            self.costed_records += 1
        }
        if u.attempt > 0 {
            add_opt(
                &mut self.repeated_usd,
                u.usd.map(|cost| if u.attempt > 1 { cost } else { 0. }),
            )
        }
    }
}
pub(super) fn pi_files(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 || dir.is_symlink() || files.len() > 10000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_symlink() {
            continue;
        }
        if p.is_dir() {
            pi_files(&p, files, depth + 1)
        } else if p.extension().is_some_and(|x| x == "jsonl") {
            files.push(p)
        }
    }
}
pub(super) fn node_at(events: &[Event], at: i64) -> (String, u32) {
    let mut last = (String::new(), String::new());
    let mut attempt = BTreeMap::<String, u32>::new();
    let mut result = (String::new(), 0);
    for e in events {
        if e.created * 1000 > at {
            break;
        }
        if e.kind != "state" {
            continue;
        }
        let state = (e.node.clone(), e.text.clone());
        if e.text == "running" && state != last {
            let count = attempt.entry(e.node.clone()).or_default();
            *count += 1;
            result = (e.node.clone(), *count)
        }
        last = state;
    }
    result
}
fn historical_pi(
    r: &Run,
    events: &[Event],
    files: &[PathBuf],
    read_bytes: &mut u64,
) -> (Vec<Usage>, usize) {
    let mut result = Vec::new();
    let mut missed = 0;
    for (binding, s) in &r.sessions {
        if s.runtime != "pi" {
            continue;
        }
        let suffix = format!("_{}.jsonl", s.native_id);
        let file = files.iter().find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(&suffix))
        });
        let Some(file) = file else {
            missed += 1;
            continue;
        };
        let Ok(meta) = file.metadata() else {
            missed += 1;
            continue;
        };
        if meta.len() > 32 * 1024 * 1024 || *read_bytes + meta.len() > 128 * 1024 * 1024 {
            missed += 1;
            continue;
        }
        *read_bytes += meta.len();
        let Ok(f) = std::fs::File::open(file) else {
            missed += 1;
            continue;
        };
        let mut valid = false;
        for line in BufReader::new(f).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if v["type"] == "session" {
                valid = v["id"].as_str() == Some(&s.native_id)
                    && v["cwd"].as_str() == Some(&r.workspace);
                continue;
            }
            if !valid || v["type"] != "message" || v["message"]["role"] != "assistant" {
                continue;
            }
            let m = &v["message"];
            let at = m["timestamp"].as_i64().unwrap_or(0);
            let (node, attempt) = node_at(events, at);
            if let Some(u) = pi_usage(m, &s.native_id, &node, binding, attempt, "native_history") {
                result.push(u)
            }
        }
        if !valid {
            missed += 1
        }
    }
    (result, missed)
}
fn event_data(app: &App, uid: i64, rid: &str) -> anyhow::Result<Vec<Event>> {
    let db = app.store.db.lock().unwrap();
    let mut q=db.prepare("SELECT seq,json FROM events WHERE user_id=? AND run=? AND json_extract(json,'$.kind') IN ('state','error','usage','agent_turn') ORDER BY seq")?;
    q.query_map(rusqlite::params![uid, rid], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })?
    .map(|row| {
        let (seq, s) = row?;
        let mut e: Event = serde_json::from_str(&s)?;
        e.seq = seq;
        Ok(e)
    })
    .collect()
}
fn wait_ms(events: &[Event], end: i64) -> u64 {
    let mut since = None;
    let mut total = 0;
    for e in events {
        if e.kind != "state" {
            continue;
        }
        let waiting = ["waiting_user", "waiting_review"].contains(&e.text.as_str());
        if waiting {
            if since.is_none() {
                since = Some(e.created)
            }
        } else if let Some(start) = since.take() {
            total += (e.created - start).max(0) as u64 * 1000;
        }
    }
    if let Some(start) = since {
        total += (end - start).max(0) as u64 * 1000
    }
    total
}
fn merge_codex_history(usage: &mut BTreeMap<String, Usage>, historic: Vec<Usage>) {
    let bindings: HashSet<String> = historic.iter().map(|u| u.binding.clone()).collect();
    for binding in bindings {
        let rows: Vec<_> = historic.iter().filter(|u| u.binding == binding).collect();
        let last = rows.iter().map(|u| u.at).max().unwrap_or(0);
        let live: Vec<_> = usage
            .values()
            .filter(|u| u.runtime == "codex" && u.binding == binding)
            .collect();
        // Do not discard authoritative money or summaries newer than a partial native log.
        if live
            .iter()
            .any(|u| u.usd.is_some() || u.at > last + 120_000)
        {
            continue;
        }
        usage.retain(|_, u| u.runtime != "codex" || u.binding != binding);
        for u in rows {
            usage.insert(u.id.clone(), u.clone());
        }
    }
}
#[cfg(test)]
pub(crate) fn report(app: &App, uid: i64) -> anyhow::Result<Value> {
    report_priced(app, uid, &crate::codex_cost::Catalog::default())
}
fn report_priced(
    app: &App,
    uid: i64,
    prices: &crate::codex_cost::Catalog,
) -> anyhow::Result<Value> {
    let runs: Vec<Run> = {
        let db = app.store.db.lock().unwrap();
        let mut q = db.prepare(
            "SELECT json FROM runs WHERE user_id=? ORDER BY json_extract(json,'$.created') DESC",
        )?;
        q.query_map([uid], |r| r.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect::<anyhow::Result<_>>()?
    };
    let directory = app.data.join(format!("users/{uid}/.pi/agent/sessions"));
    let home = app.data.join(format!("users/{uid}"));
    let mut files = Vec::new();
    if !directory
        .ancestors()
        .take_while(|p| p.starts_with(&app.data))
        .any(|p| p.is_symlink())
    {
        pi_files(&directory, &mut files, 0)
    }
    let mut codex_files = Vec::new();
    let codex_dir = home.join(".codex/sessions");
    if !codex_dir
        .ancestors()
        .take_while(|p| p.starts_with(&app.data))
        .any(|p| p.is_symlink())
    {
        pi_files(&codex_dir, &mut codex_files, 0);
    }
    let claude_files = crate::claude_time::files(&home);
    let mut codex_missed = 0;
    let mut total = Metrics::default();
    let mut workflows = BTreeMap::<String, Metrics>::new();
    let mut models = BTreeMap::<String, Metrics>::new();
    let mut agents = BTreeMap::<String, Metrics>::new();
    let mut names: BTreeMap<String, String> = app
        .store
        .workflows(uid)?
        .into_iter()
        .map(|w| (w.id, w.name))
        .collect();
    let mut run_rows = Vec::new();
    let mut completed = 0;
    let mut failed = 0;
    let mut waiting = 0;
    let mut active = 0;
    let mut steps = 0u64;
    let mut repetitions = 0u64;
    let mut reworks = 0u64;
    let mut decisions = 0u64;
    let mut sessions = 0;
    let mut covered = 0;
    let mut read_bytes = 0;
    let mut historical_missed = 0;
    for r in &runs {
        let events = event_data(app, uid, &r.id)?;
        let mut usage = BTreeMap::<String, Usage>::new();
        let (historic, missed) = historical_pi(r, &events, &files, &mut read_bytes);
        historical_missed += missed;
        for u in historic {
            usage.insert(u.id.clone(), u);
        }
        for e in &events {
            if e.kind == "usage"
                && let Ok(u) = serde_json::from_str::<Usage>(&e.text)
            {
                usage.insert(u.id.clone(), u);
            }
        }
        let (historic, missed) =
            crate::codex_cost::history(r, &events, &codex_files, &mut read_bytes);
        codex_missed += missed;
        if !["running", "starting"].contains(&r.status.as_str()) {
            merge_codex_history(&mut usage, historic);
        }
        crate::claude_time::enrich(r, &claude_files, &mut usage, &mut read_bytes, None);
        let mut metrics = Metrics::default();
        let mut nodes = BTreeMap::<String, Metrics>::new();
        let mut daily = BTreeMap::<i64, Metrics>::new();
        for u in usage.values() {
            metrics.add_priced(u, prices);
            total.add_priced(u, prices);
            workflows
                .entry(r.workflow.id.clone())
                .or_default()
                .add_priced(u, prices);
            models
                .entry(format!("{} · {}", u.runtime, u.model))
                .or_default()
                .add_priced(u, prices);
            agents
                .entry(format!(
                    "{} · {} · {}",
                    names.get(&r.workflow.id).unwrap_or(&r.workflow.name),
                    r.workflow.id.chars().take(8).collect::<String>(),
                    u.binding
                ))
                .or_default()
                .add_priced(u, prices);
            nodes
                .entry(u.node.clone())
                .or_default()
                .add_priced(u, prices);
            daily
                .entry(u.at / 86400000 * 86400000)
                .or_default()
                .add_priced(u, prices);
        }
        for e in &events {
            if e.kind == "agent_turn"
                && let Ok(v) = serde_json::from_str::<Value>(&e.text)
            {
                let duration = v["duration_ms"].as_u64();
                add_opt(&mut metrics.agent_ms, duration);
                add_opt(&mut total.agent_ms, duration);
                add_opt(
                    &mut nodes
                        .entry(v["node"].as_str().unwrap_or("").into())
                        .or_default()
                        .agent_ms,
                    duration,
                );
                let tools = n(&v, "tools");
                metrics.tools += tools;
                total.tools += tools;
                nodes
                    .entry(v["node"].as_str().unwrap_or("").into())
                    .or_default()
                    .tools += tools;
            }
        }
        sessions += r.sessions.len();
        let bindings: HashSet<_> = usage.values().map(|u| u.binding.clone()).collect();
        covered += r.sessions.keys().filter(|k| bindings.contains(*k)).count();
        let run_steps = r.attempts.values().map(|n| *n as u64).sum::<u64>();
        let repeats = r
            .attempts
            .values()
            .map(|n| n.saturating_sub(1) as u64)
            .sum::<u64>();
        let rework = r.reworks.values().map(|n| *n as u64).sum::<u64>();
        let ds = app.store.decisions(uid, &r.id)?;
        decisions += ds.len() as u64;
        steps += run_steps;
        repetitions += repeats;
        reworks += rework;
        if r.status == "completed" {
            completed += 1
        }
        if ["failed", "interrupted"].contains(&r.status.as_str()) {
            failed += 1
        }
        if ["waiting_user", "waiting_review"].contains(&r.status.as_str()) {
            waiting += 1
        }
        if ["running", "starting", "waiting_external"].contains(&r.status.as_str()) {
            active += 1
        }
        let artifact_count: i64 = app.store.db.lock().unwrap().query_row(
            "SELECT count(*) FROM artifacts WHERE user_id=? AND run=?",
            rusqlite::params![uid, r.id],
            |r| r.get(0),
        )?;
        names
            .entry(r.workflow.id.clone())
            .or_insert_with(|| r.workflow.name.clone());
        workflows.entry(r.workflow.id.clone()).or_default();
        let end = if ["completed", "failed", "interrupted"].contains(&r.status.as_str()) {
            r.updated
        } else {
            now()
        };
        let mut node_rows:Vec<_>=r.workflow.nodes.iter().filter(|n|r.attempts.contains_key(&n.id)||nodes.contains_key(&n.id)).map(|n|json!({"id":n.id,"name":n.title,"kind":n.kind,"status":r.states.get(&n.id),"attempts":r.attempts.get(&n.id).unwrap_or(&0),"metrics":nodes.get(&n.id).cloned().unwrap_or_default()})).collect();
        for (id, metrics) in &nodes {
            if !r.workflow.nodes.iter().any(|n| &n.id == id) {
                node_rows.push(json!({"id":id,"name":"Non attribuito","kind":"","status":null,"attempts":0,"metrics":metrics}));
            }
        }
        run_rows.push(json!({"id":r.id,"parent":r.parent,"children":r.children,"name":r.workflow.name,"workflow_id":r.workflow.id,"status":r.status,"created":r.created,"updated":r.updated,"steps":run_steps,"repetitions":repeats,"reworks":rework,"decisions":ds.len(),"approvals":ds.iter().filter(|d|d.decision=="approve").count(),"rejections":ds.iter().filter(|d|d.decision=="reject").count(),"errors":events.iter().filter(|e|e.kind=="error").count(),"artifacts":artifact_count,"elapsed_ms":(end-r.created).max(0) as u64*1000,"human_wait_ms":wait_ms(&events.iter().filter(|e|!r.workflow.nodes.iter().any(|n|n.id==e.node&&n.kind=="Workflow")).cloned().collect::<Vec<_>>(),end),"sessions":r.sessions.len(),"sessions_with_usage":bindings.len(),"metrics":metrics,"nodes":node_rows,"daily":daily.iter().map(|(at,m)|json!({"at":at,"metrics":m})).collect::<Vec<_>>() }));
    }
    // Inclusive totals are exposed separately; global/workflow aggregates remain exclusive.
    let own: BTreeMap<String, Option<f64>> = run_rows
        .iter()
        .map(|r| {
            (
                r["id"].as_str().unwrap().into(),
                r["metrics"]["usd"].as_f64(),
            )
        })
        .collect();
    fn inclusive(
        id: &str,
        runs: &[Run],
        own: &BTreeMap<String, Option<f64>>,
        seen: &mut HashSet<String>,
    ) -> Option<f64> {
        if !seen.insert(id.into()) {
            return None;
        }
        let mut total = own.get(id).copied().flatten();
        if let Some(run) = runs.iter().find(|r| r.id == id) {
            for call in &run.children {
                add_opt(&mut total, inclusive(&call.run, runs, own, seen));
            }
        }
        total
    }
    for row in &mut run_rows {
        let rid = row["id"].as_str().unwrap().to_string();
        row["tree_usd"] = json!(inclusive(&rid, &runs, &own, &mut HashSet::new()));
        let run = runs.iter().find(|r| r.id == rid).unwrap();
        for node in row["nodes"].as_array_mut().unwrap() {
            if node["kind"] == "Workflow" {
                let mut usd = None;
                let mut seen = HashSet::new();
                for call in run
                    .children
                    .iter()
                    .filter(|c| Some(c.node.as_str()) == node["id"].as_str())
                {
                    add_opt(&mut usd, inclusive(&call.run, &runs, &own, &mut seen));
                }
                node["child_usd"] = json!(usd);
            }
        }
    }
    for (metric, tree, child) in [
        ("estimated_usd", "tree_estimated_usd", "child_estimated_usd"),
        ("total_usd", "tree_total_usd", "child_total_usd"),
    ] {
        let own = run_rows
            .iter()
            .map(|r| {
                (
                    r["id"].as_str().unwrap().to_string(),
                    r["metrics"][metric].as_f64(),
                )
            })
            .collect();
        for row in &mut run_rows {
            let rid = row["id"].as_str().unwrap().to_string();
            row[tree] = json!(inclusive(&rid, &runs, &own, &mut HashSet::new()));
            let run = runs.iter().find(|r| r.id == rid).unwrap();
            for node in row["nodes"].as_array_mut().unwrap() {
                if node["kind"] != "Workflow" {
                    continue;
                }
                let mut usd = None;
                let mut seen = HashSet::new();
                for call in run
                    .children
                    .iter()
                    .filter(|c| Some(c.node.as_str()) == node["id"].as_str())
                {
                    add_opt(&mut usd, inclusive(&call.run, &runs, &own, &mut seen));
                }
                node[child] = json!(usd);
            }
        }
    }
    // Manager turns use their own conversation IDs, outside the runs table.
    let workflow_metrics = serde_json::to_value(&total)?;
    let manager_events: Vec<Event> = {
        let db = app.store.db.lock().unwrap();
        let mut q = db.prepare("SELECT json FROM events WHERE user_id=? AND json_extract(json,'$.kind') IN ('usage','agent_turn') AND json_extract(json,'$.node')='manager' ORDER BY seq")?;
        q.query_map([uid], |r| r.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect::<anyhow::Result<_>>()?
    };
    let mut manager_usage = BTreeMap::new();
    for e in &manager_events {
        if e.kind == "usage"
            && let Ok(u) = serde_json::from_str::<Usage>(&e.text)
            && u.binding == crate::manager::BINDING
        {
            manager_usage.insert(u.id.clone(), u);
        }
    }
    let chats: Vec<String> = {
        let db = app.store.db.lock().unwrap();
        let mut q = db.prepare("SELECT json FROM manager_chats WHERE user_id=?")?;
        q.query_map([uid], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    // Each chat has a separate workspace/session; merge all recovered requests together.
    let mut manager_history = Vec::new();
    for raw in chats {
        let v: Value = serde_json::from_str(&raw)?;
        let Ok(mut run) = serde_json::from_value::<Run>(v["runtime"].clone()) else {
            continue;
        };
        let Some(id) = v["id"]
            .as_str()
            .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        else {
            continue;
        };
        if Path::new(&run.workspace) != home.join("manager").join(id) {
            continue;
        }
        if let Some(retired) = v["retired_sessions"].as_array() {
            for (i, session) in retired.iter().enumerate() {
                if let Ok(session) = serde_json::from_value::<NativeSession>(session.clone()) {
                    run.sessions.insert(format!("retired:{i}"), session);
                }
            }
        }
        crate::claude_time::enrich(
            &run,
            &claude_files,
            &mut manager_usage,
            &mut read_bytes,
            Some(crate::manager::BINDING),
        );
        let (mut history, missed) =
            crate::codex_cost::history(&run, &[], &codex_files, &mut read_bytes);
        codex_missed += missed;
        for u in &mut history {
            u.binding = crate::manager::BINDING.into();
            u.node = "manager".into();
        }
        manager_history.extend(history);
    }
    // Old manager summaries have no native session ID. Never replace them with a
    // potentially partial collection from several chats sharing the same binding.
    if !manager_usage.values().any(|u| u.runtime == "codex") {
        for u in manager_history {
            manager_usage.insert(u.id.clone(), u);
        }
    }
    let manager_events = manager_events
        .into_iter()
        .filter(|e| e.kind != "usage")
        .chain(manager_usage.values().map(|u| Event {
            seq: 0,
            run: String::new(),
            kind: "usage".into(),
            node: "manager".into(),
            text: serde_json::to_string(u).unwrap(),
            created: u.at / 1000,
        }))
        .collect::<Vec<_>>();
    let mut manager = Metrics::default();
    let mut manager_daily = BTreeMap::<i64, Metrics>::new();
    let mut manager_seen = HashSet::new();
    let mut manager_turns = 0u64;
    for e in manager_events {
        let Ok(v) = serde_json::from_str::<Value>(&e.text) else {
            continue;
        };
        if v["binding"] != crate::manager::BINDING {
            continue;
        }
        if e.kind == "usage" {
            let Ok(u) = serde_json::from_value::<Usage>(v) else {
                continue;
            };
            if !manager_seen.insert(format!("usage:{}", u.id)) {
                continue;
            }
            manager.add_priced(&u, prices);
            total.add_priced(&u, prices);
            manager_daily
                .entry(u.at / 86400000 * 86400000)
                .or_default()
                .add_priced(&u, prices);
            models
                .entry(format!("{} · {}", u.runtime, u.model))
                .or_default()
                .add_priced(&u, prices);
            agents
                .entry(format!("Manager · {} · {}", u.runtime, u.model))
                .or_default()
                .add_priced(&u, prices);
        } else {
            let Some(id) = v["id"].as_str() else { continue };
            if !manager_seen.insert(format!("turn:{id}")) {
                continue;
            }
            manager_turns += 1;
            let duration = v["duration_ms"].as_u64();
            add_opt(&mut manager.agent_ms, duration);
            add_opt(&mut total.agent_ms, duration);
            manager.tools += n(&v, "tools");
            total.tools += n(&v, "tools");
        }
    }
    let manager_report = json!({"metrics":manager,"turns":manager_turns,"daily":manager_daily.iter().map(|(at,m)|json!({"at":at,"metrics":m})).collect::<Vec<_>>()});
    let workflow_rows:Vec<_>=workflows.iter().map(|(id,m)|json!({"id":id,"name":names.get(id),"runs":runs.iter().filter(|r|&r.workflow.id==id).count(),"completed":runs.iter().filter(|r|&r.workflow.id==id&&r.status=="completed").count(),"metrics":m})).collect();
    let model_rows: Vec<_> = models
        .iter()
        .map(|(name, m)| json!({"name":name,"metrics":m}))
        .collect();
    let agent_rows: Vec<_> = agents
        .iter()
        .map(|(name, m)| json!({"name":name,"metrics":m}))
        .collect();
    Ok(
        json!({"pricing":prices.metadata(),"generated":now(),"scope":"current_user","currency":"USD","manager":manager_report,"summary":{"workflow_metrics":workflow_metrics,"runs":runs.len(),"completed":completed,"failed":failed,"waiting":waiting,"active":active,"steps":steps,"repetitions":repetitions,"reworks":reworks,"decisions":decisions,"sessions":sessions,"sessions_with_usage":covered,"historical_sessions_unavailable":historical_missed,"historical_codex_sessions_unavailable":codex_missed,"metrics":total},"workflows":workflow_rows,"models":model_rows,"agents":agent_rows,"runs":run_rows}),
    )
}
pub async fn get(State(app): State<App>, u: User) -> ApiResult<Json<Value>> {
    let prices = crate::codex_cost::catalog(&app).await;
    Ok(Json(
        tokio::task::spawn_blocking(move || report_priced(&app, u.id, &prices))
            .await
            .map_err(|_| bad("Statistiche FinOps non disponibili"))??,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message() -> Value {
        json!({"role":"assistant","timestamp":1790000000000i64,"responseId":"response-1","model":"test-model","content":[{"type":"toolCall","id":"tool-1"}],"durationMs":10,"usage":{"input":10,"output":5,"cacheRead":20,"cacheWrite":30,"reasoning":2,"cost":{"total":0.01}}})
    }
    #[test]
    fn normalizes_caches_and_deduplicates_native_events() {
        let mut t = Tracker::new("pi", "task", "author", "alias", 1);
        let v = json!({"type":"message_end","message":message()});
        let u = t.observe(&v, "sid").unwrap();
        assert_eq!(u.input + u.output, 65);
        assert_eq!(u.reasoning, Some(2));
        assert_eq!(u.tools, 1);
        assert!(t.observe(&v, "sid").is_none());
        let mut m = Metrics::default();
        m.add(&u);
        assert_eq!(m.total, 65);
        assert_eq!(m.repeated_usd, Some(0.));
        let mut c = Tracker::new("codex", "task", "author", "model", 1);
        let u=c.observe(&json!({"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":20,"output_tokens_details":{"reasoning_tokens":10}}}),"sid").unwrap();
        assert_eq!(u.input + u.output, 120);
        assert_eq!(u.usd, None);
        assert_eq!(u.calls, None);
    }
    #[test]
    fn claude_does_not_double_count_cache_or_tool_events() {
        let mut t = Tracker::new("claude", "task", "author", "model", 2);
        let tool = json!({"type":"assistant","message":{"id":"message-1","content":[{"type":"tool_use","id":"tool-1"}]}});
        t.observe(&tool, "sid");
        t.observe(&tool, "sid");
        let u=t.observe(&json!({"type":"result","total_cost_usd":0.5,"usage":{"input_tokens":100,"output_tokens":5,"cache_read_input_tokens":20,"cache_creation_input_tokens":10}}),"sid").unwrap();
        assert_eq!(u.input + u.output, 135);
        assert_eq!(u.calls, Some(1));
        assert_eq!(t.finish(10, "cli_success")["tools"], 1);
        let mut m = Metrics::default();
        m.add(&u);
        assert_eq!(m.repeated_usd, Some(0.5));
    }
    #[test]
    fn waiting_duration_does_not_double_count_duplicate_state_events() {
        let e = |text: &str, created| Event {
            seq: 0,
            run: "r".into(),
            kind: "state".into(),
            node: "task".into(),
            text: text.into(),
            created,
        };
        assert_eq!(
            wait_ms(
                &[
                    e("waiting_user", 10),
                    e("waiting_user", 10),
                    e("running", 20),
                    e("running", 20)
                ],
                25
            ),
            10000
        );
        assert_eq!(
            node_at(
                &[
                    e("running", 10),
                    e("running", 10),
                    e("waiting_user", 20),
                    e("running", 30)
                ],
                31000
            ),
            ("task".into(), 2)
        );
    }
    #[tokio::test]
    async fn historical_usage_is_owned_and_not_duplicated_by_live_collection() {
        let d = tempfile::tempdir().unwrap();
        let app = crate::tests::app(d.path());
        let w = crate::presets::all().remove(0);
        let mut r = crate::engine::create(&app, 1, w.clone(), "test".into(), None)
            .await
            .unwrap();
        let _other = crate::engine::create(&app, 2, w, "other".into(), None)
            .await
            .unwrap();
        let sid = crate::model::id();
        r.sessions.insert(
            "author".into(),
            NativeSession {
                runtime: "pi".into(),
                native_id: sid.clone(),
                turns: 1,
                model: "alias".into(),
            },
        );
        app.store.save_run(1, &r).unwrap();
        let p = app.data.join("users/1/.pi/agent/sessions");
        std::fs::create_dir_all(&p).unwrap();
        let m = message();
        let content = format!(
            "{}\n{}\n",
            json!({"type":"session","id":sid,"cwd":r.workspace}),
            json!({"type":"message","message":m})
        );
        std::fs::write(p.join(format!("test_{sid}.jsonl")), content).unwrap();
        let u = pi_usage(&m, &sid, "constitution", "author", 1, "live").unwrap();
        app.store
            .event(
                1,
                &r.id,
                "usage",
                "constitution",
                &serde_json::to_string(&u).unwrap(),
            )
            .unwrap();
        let value = report(&app, 1).unwrap();
        assert_eq!(value["summary"]["metrics"]["records"], 1);
        assert_eq!(value["summary"]["metrics"]["total"], 65);
        assert_eq!(value["summary"]["metrics"]["usd"], 0.01);
        let other = report(&app, 2).unwrap();
        assert_eq!(other["summary"]["metrics"]["records"], 0);
        assert!(other["summary"]["metrics"]["usd"].is_null());
        assert_ne!(other["runs"][0]["id"], r.id);
    }
    #[tokio::test]
    async fn codex_estimates_and_history_are_separate_owned_and_not_duplicated() {
        let d = tempfile::tempdir().unwrap();
        let app = crate::tests::app(d.path());
        let mut r = crate::engine::create(
            &app,
            1,
            crate::presets::all().remove(0),
            "cost".into(),
            None,
        )
        .await
        .unwrap();
        let sid = crate::model::id();
        r.status = "completed".into();
        r.sessions.insert(
            "author".into(),
            NativeSession {
                runtime: "codex".into(),
                native_id: sid.clone(),
                model: "gpt-test".into(),
                turns: 1,
            },
        );
        app.store.save_run(1, &r).unwrap();
        let dir = app.data.join("users/1/.codex/sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let token = json!({"type":"event_msg","timestamp":"2026-10-31T10:00:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20},"last_token_usage":{"input_tokens":100,"output_tokens":20,"cached_input_tokens":40,"cache_write_input_tokens":10}}}});
        let header = json!({"type":"session_meta","payload":{"id":sid,"cwd":r.workspace}});
        std::fs::write(
            dir.join(format!("rollout-{sid}.jsonl")),
            format!("{header}\n{token}\n{token}\n"),
        )
        .unwrap();
        let mut tracker = Tracker::new("codex", "task", "author", "gpt-test", 1);
        let mut u=tracker.observe(&json!({"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":20,"cached_input_tokens":40}}),&sid).unwrap();
        u.at = chrono::DateTime::parse_from_rfc3339("2026-10-31T10:00:01Z")
            .unwrap()
            .timestamp_millis();
        app.store
            .event(
                1,
                &r.id,
                "usage",
                "task",
                &serde_json::to_string(&u).unwrap(),
            )
            .unwrap();
        let prices=crate::codex_cost::Catalog::parse(&json!({"data":[{"id":"openai/gpt-test","pricing":{"prompt":"0.01","completion":"0.1","input_cache_read":"0.001","input_cache_write":"0.02"}}]})).unwrap();
        let value = report_priced(&app, 1, &prices).unwrap();
        let m = &value["summary"]["metrics"];
        assert_eq!(m["records"], 1);
        assert_eq!(m["input"], 100);
        assert!(m["usd"].is_null());
        assert!((m["estimated_usd"].as_f64().unwrap() - 2.74).abs() < 1e-10);
        assert_eq!(m["estimated_records"], 1);
        assert_eq!(m["approximate_records"], 0);
        assert_eq!(m["historical_codex_records"], 1);
        assert_eq!(
            report_priced(&app, 2, &prices).unwrap()["summary"]["metrics"]["records"],
            0
        );
        // A newer live summary must survive an incomplete/older historical file.
        u.at += 300_000;
        let mut usage = BTreeMap::from([(u.id.clone(), u.clone())]);
        let mut old = u.clone();
        old.id = "history".into();
        old.at -= 300_000;
        old.source = "codex_native_history".into();
        merge_codex_history(&mut usage, vec![old]);
        assert!(usage.contains_key(&u.id));
        std::fs::remove_dir_all(&dir).unwrap();
        let fallback = report_priced(&app, 1, &prices).unwrap();
        assert_eq!(fallback["summary"]["metrics"]["approximate_records"], 1);
        assert_eq!(
            fallback["summary"]["historical_codex_sessions_unavailable"],
            1
        );
    }
    #[test]
    fn manager_consumption_is_owned_deduplicated_and_separate_from_runs() {
        let d = tempfile::tempdir().unwrap();
        let app = crate::tests::app(d.path());
        let mut u = pi_usage(
            &message(),
            "manager-session",
            "manager",
            crate::manager::BINDING,
            1,
            "live",
        )
        .unwrap();
        for uid in [1, 1, 2] {
            app.store
                .event(
                    uid,
                    "conversation",
                    "usage",
                    "manager",
                    &serde_json::to_string(&u).unwrap(),
                )
                .unwrap();
        }
        let turn =
            json!({"id":"turn-1","binding":crate::manager::BINDING,"duration_ms":120,"tools":2});
        for _ in 0..2 {
            app.store
                .event(
                    1,
                    "conversation",
                    "agent_turn",
                    "manager",
                    &turn.to_string(),
                )
                .unwrap();
        }
        // An ordinary workflow node named manager must not enter manager totals.
        u.id = "other".into();
        u.binding = "author".into();
        app.store
            .event(
                1,
                "unrelated",
                "usage",
                "manager",
                &serde_json::to_string(&u).unwrap(),
            )
            .unwrap();
        let value = report(&app, 1).unwrap();
        assert_eq!(value["manager"]["metrics"]["records"], 1);
        assert_eq!(value["summary"]["metrics"]["total"], 65);
        assert_eq!(value["summary"]["metrics"]["usd"], 0.01);
        assert_eq!(value["manager"]["turns"], 1);
        assert_eq!(value["manager"]["metrics"]["agent_ms"], 120);
        assert_eq!(value["manager"]["metrics"]["tools"], 3);
        assert_eq!(value["summary"]["runs"], 0);
        assert!(value["summary"]["workflow_metrics"]["usd"].is_null());
        assert_eq!(value["models"][0]["metrics"]["total"], 65);
        assert_eq!(value["manager"]["daily"].as_array().unwrap().len(), 1);
        assert_eq!(report(&app, 2).unwrap()["manager"]["turns"], 0);
        assert_eq!(report(&app, 3).unwrap()["summary"]["metrics"]["records"], 0);
    }
}
