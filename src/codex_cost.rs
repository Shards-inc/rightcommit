//! Public OpenRouter prices, used only as an API-equivalent estimate.
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Default)]
pub struct Catalog {
    pub at: i64,
    pub prices: BTreeMap<String, Value>,
    pub stale: bool,
    pub persisted: bool,
}
fn rate(v: &Value) -> Option<f64> {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| v.as_f64())
        .filter(|n: &f64| n.is_finite() && *n >= 0.)
}
impl Catalog {
    pub fn parse(v: &Value) -> Option<Self> {
        let mut result = Self {
            at: crate::model::now(),
            ..Self::default()
        };
        for m in v["data"].as_array()? {
            let Some(id) = m["id"].as_str().filter(|s| s.starts_with("openai/")) else {
                continue;
            };
            let p = &m["pricing"];
            if rate(&p["prompt"]).is_some() && rate(&p["completion"]).is_some() {
                result.prices.insert(id.into(), p.clone());
            }
        }
        (!result.prices.is_empty()).then_some(result)
    }
    pub fn metadata(&self) -> Value {
        json!({"source":"https://openrouter.ai/api/v1/models","fetched_at":self.at,
            "available":!self.prices.is_empty(),"stale":self.stale,"persisted":self.persisted,"prices":self.prices,
            "basis":"current_catalog_api_equivalent"})
    }
    pub fn estimate(&self, u: &crate::finops::Usage) -> Option<f64> {
        if u.runtime != "codex" || u.usd.is_some() {
            return None;
        }
        // Exact IDs only. Do not guess prices for misspellings, aliases or other providers.
        let id = if u.model.starts_with("openai/") {
            u.model.clone()
        } else {
            format!("openai/{}", u.model)
        };
        let base = self.prices.get(&id)?;
        let mut p = base.clone();
        if let Some(overrides) = base.get("overrides") {
            for tier in overrides.as_array()? {
                // Unsupported time/provider pricing cannot safely be treated as a context tier.
                if tier.as_object()?.keys().any(|k| {
                    ![
                        "min_prompt_tokens",
                        "prompt",
                        "completion",
                        "input_cache_read",
                        "input_cache_write",
                    ]
                    .contains(&k.as_str())
                }) {
                    return None;
                }
                let min = tier["min_prompt_tokens"].as_u64()?;
                if u.source == "codex_native_history" && u.input > min {
                    for (key, value) in tier.as_object()? {
                        if key != "min_prompt_tokens" {
                            p[key] = value.clone();
                        }
                    }
                }
            }
        }
        let read = u.cache_read?;
        let write = u.cache_write.unwrap_or(0);
        let ordinary = u.input.checked_sub(read)?.checked_sub(write)?;
        let mut cost =
            ordinary as f64 * rate(&p["prompt"])? + u.output as f64 * rate(&p["completion"])?;
        if read > 0 {
            cost += read as f64 * rate(&p["input_cache_read"])?;
        }
        if write > 0 {
            cost += write as f64 * rate(&p["input_cache_write"])?;
        }
        cost.is_finite().then_some(cost)
    }
}

#[derive(Default)]
struct Cache {
    attempted_at: i64,
    catalog: Catalog,
}
impl Cache {
    fn load(store: &crate::store::Store) -> anyhow::Result<Self> {
        use rusqlite::OptionalExtension;
        let raw: Option<String> = store
            .db
            .lock()
            .unwrap()
            .query_row("SELECT json FROM finops_catalog WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        let Some(raw) = raw.filter(|s| s.len() <= 8 * 1024 * 1024) else {
            return Ok(Self::default());
        };
        let snapshot: Value = serde_json::from_str(&raw)?;
        anyhow::ensure!(snapshot["version"] == 1, "Formato cache tariffe non valido");
        let at = snapshot["fetched_at"].as_i64().unwrap_or(0);
        let mut catalog = if at == 0 {
            Catalog::default()
        } else {
            anyhow::ensure!(
                at > 0 && at <= crate::model::now() + 300,
                "Data cache tariffe non valida"
            );
            Catalog::parse(&snapshot).ok_or_else(|| anyhow::anyhow!("Cache tariffe non valida"))?
        };
        catalog.at = at; // Never replace the price date with the restart/load time.
        catalog.stale = snapshot["stale"].as_bool().unwrap_or(true);
        catalog.persisted = !catalog.prices.is_empty();
        Ok(Self {
            attempted_at: snapshot["attempted_at"].as_i64().unwrap_or(0),
            catalog,
        })
    }
    fn fresh(&self, now: i64) -> bool {
        self.attempted_at > 0 && now >= self.attempted_at && now - self.attempted_at < 300
    }
    fn update(&mut self, store: &crate::store::Store, now: i64, fetched: Option<Catalog>) {
        self.attempted_at = now;
        if let Some(catalog) = fetched {
            self.catalog = catalog;
        } else {
            self.catalog.stale = true;
        }
        let snapshot = json!({"version":1,"fetched_at":self.catalog.at,"attempted_at":now,"stale":self.catalog.stale,
            "data":self.catalog.prices.iter().map(|(id,pricing)|json!({"id":id,"pricing":pricing})).collect::<Vec<_>>()});
        // One atomic SQLite write preserves the last complete snapshot across restarts.
        let saved = store.db.lock().unwrap().execute(
            "INSERT INTO finops_catalog(id,json) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
            [snapshot.to_string()]
        );
        self.catalog.persisted = saved.is_ok() && !self.catalog.prices.is_empty();
        if let Err(error) = saved {
            tracing::warn!(%error, "Impossibile salvare la cache delle tariffe OpenRouter");
        }
    }
}

pub async fn catalog(app: &crate::App) -> Catalog {
    static CACHE: OnceLock<tokio::sync::Mutex<BTreeMap<std::path::PathBuf, Cache>>> =
        OnceLock::new();
    let mut caches = CACHE
        .get_or_init(|| tokio::sync::Mutex::new(BTreeMap::new()))
        .lock()
        .await;
    let cache = caches.entry(app.data.clone()).or_insert_with(|| {
        Cache::load(&app.store).unwrap_or_else(|error| {
            tracing::warn!(%error, "Cache delle tariffe OpenRouter non disponibile");
            Cache::default()
        })
    });
    let now = crate::model::now();
    if cache.fresh(now) {
        return cache.catalog.clone();
    }
    let fetched = async {
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .ok()?
            .get("https://openrouter.ai/api/v1/models")
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        Catalog::parse(&serde_json::from_slice::<Value>(&bytes).ok()?)
    }
    .await;
    cache.update(&app.store, now, fetched);
    cache.catalog.clone()
}

/// Only sessions explicitly retained by this user's run may be inspected.
pub fn history(
    r: &crate::model::Run,
    events: &[crate::model::Event],
    files: &[std::path::PathBuf],
    budget: &mut u64,
) -> (Vec<crate::finops::Usage>, usize) {
    use std::io::{BufRead, BufReader};
    let mut result = Vec::new();
    let mut missed = 0;
    for (binding, session) in &r.sessions {
        if session.runtime != "codex" {
            continue;
        }
        let sid = &session.native_id;
        if uuid::Uuid::parse_str(sid).is_err() {
            missed += 1;
            continue;
        }
        let suffix = format!("{sid}.jsonl");
        let Some(path) = files.iter().find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(&suffix))
        }) else {
            missed += 1;
            continue;
        };
        let Ok(meta) = path.metadata() else {
            missed += 1;
            continue;
        };
        if meta.len() > 32 * 1024 * 1024 || *budget + meta.len() > 128 * 1024 * 1024 {
            missed += 1;
            continue;
        }
        *budget += meta.len();
        let Ok(file) = std::fs::File::open(path) else {
            missed += 1;
            continue;
        };
        let mut valid = false;
        let mut failed = false;
        let mut model = session.model.clone();
        let mut turn = String::new();
        let mut seen = std::collections::HashSet::new();
        let mut rows = Vec::new();
        let mut open_turn = false;
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else {
                failed = true;
                break;
            };
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                failed = true;
                break;
            };
            let p = &v["payload"];
            if v["type"] == "session_meta" {
                valid = p["id"].as_str() == Some(sid) && p["cwd"].as_str() == Some(&r.workspace);
                if !valid {
                    failed = true;
                    break;
                }
            }
            if !valid {
                continue;
            }
            if v["type"] == "turn_context" {
                if let Some(m) = p["model"].as_str() {
                    model = m.into();
                }
                turn = p["turn_id"].as_str().unwrap_or("").into();
            }
            if v["type"] == "event_msg" && p["type"] == "task_started" {
                open_turn = true;
            }
            if v["type"] == "event_msg" && p["type"] == "task_complete" {
                open_turn = false;
            }
            if v["type"] != "event_msg" || p["type"] != "token_count" {
                continue;
            }
            let info = &p["info"];
            if info.is_null() {
                continue;
            }
            let usage = &info["last_token_usage"];
            let total = &info["total_token_usage"];
            if total.is_null()
                || !usage["input_tokens"].is_u64()
                || !usage["output_tokens"].is_u64()
            {
                failed = true;
                break;
            }
            // Codex can repeat the same token_count while publishing rate-limit updates.
            let key = serde_json::to_string(total).unwrap_or_default();
            if !seen.insert(key.clone()) {
                continue;
            }
            let Some(at) = v["timestamp"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp_millis())
            else {
                failed = true;
                break;
            };
            let (node, attempt) = crate::finops::node_at(events, at);
            rows.push(crate::finops::Usage {
                id: crate::store::hash(format!("codex:{sid}:{turn}:{key}")),
                node,
                binding: binding.clone(),
                runtime: "codex".into(),
                model: model.clone(),
                attempt,
                at,
                input: usage["input_tokens"].as_u64().unwrap(),
                output: usage["output_tokens"].as_u64().unwrap(),
                cache_read: usage["cached_input_tokens"].as_u64(),
                cache_write: usage["cache_write_input_tokens"].as_u64(),
                reasoning: usage["reasoning_output_tokens"].as_u64(),
                usd: None,
                llm_ms: None,
                calls: Some(1),
                tools: 0,
                source: "codex_native_history".into(),
            });
        }
        if failed || !valid || open_turn || rows.is_empty() {
            missed += 1;
        } else {
            result.extend(rows);
        }
    }
    (result, missed)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> crate::finops::Usage {
        serde_json::from_value(json!({"id":"u","node":"n","binding":"author","runtime":"codex","model":"gpt-test","attempt":1,"at":1,"input":100,"output":20,"cache_read":40,"cache_write":10,"reasoning":5,"usd":null,"llm_ms":null,"calls":1,"tools":0,"source":"codex_native_history"})).unwrap()
    }
    pub fn prices() -> Catalog {
        Catalog::parse(&json!({"data":[{"id":"openai/gpt-test","pricing":{"prompt":"0.01","completion":"0.1","input_cache_read":"0.001","input_cache_write":"0.02","overrides":[{"min_prompt_tokens":100,"prompt":"0.02","completion":"0.15","input_cache_read":"0.002","input_cache_write":"0.04"}]}}]})).unwrap()
    }
    #[test]
    fn persisted_prices_survive_reopen_and_offline_refresh_with_original_date() {
        let d = tempfile::tempdir().unwrap();
        let now = crate::model::now();
        let date = now - 3600;
        {
            let store = crate::store::Store::open(d.path()).unwrap();
            let mut cache = Cache::default();
            let mut catalog = prices();
            catalog.at = date;
            cache.update(&store, now, Some(catalog));
            assert!(cache.catalog.persisted);
        }
        {
            let store = crate::store::Store::open(d.path()).unwrap();
            let mut restarted = Cache::load(&store).unwrap();
            assert_eq!(restarted.catalog.at, date);
            assert!(restarted.fresh(now + 1));
            assert!(!restarted.fresh(now + 301));
            let before = restarted.catalog.estimate(&sample()).unwrap();
            restarted.update(&store, now + 301, None);
            assert!(restarted.catalog.stale);
            assert_eq!(restarted.catalog.at, date);
            assert_eq!(restarted.catalog.estimate(&sample()), Some(before));
        }
        let store = crate::store::Store::open(d.path()).unwrap();
        let again = Cache::load(&store).unwrap();
        assert!(again.catalog.persisted && again.catalog.stale);
        assert_eq!(again.catalog.at, date);
        assert!(again.fresh(now + 302));
        assert!(!again.fresh(now)); // a clock rollback must not freeze refreshes
        assert!(again.catalog.estimate(&sample()).is_some());
    }
    #[test]
    fn missing_or_corrupt_persistent_prices_never_become_zero_costs() {
        let d = tempfile::tempdir().unwrap();
        let store = crate::store::Store::open(d.path()).unwrap();
        let mut cache = Cache::load(&store).unwrap();
        assert!(cache.catalog.estimate(&sample()).is_none());
        cache.update(&store, crate::model::now(), None);
        let offline = Cache::load(&store).unwrap();
        assert!(offline.catalog.stale);
        assert!(offline.catalog.prices.is_empty());
        assert!(offline.catalog.estimate(&sample()).is_none());
        store
            .db
            .lock()
            .unwrap()
            .execute("UPDATE finops_catalog SET json='broken'", [])
            .unwrap();
        assert!(Cache::load(&store).is_err());
    }
    #[test]
    fn prices_cache_context_threshold_and_unknown_models() {
        let c = prices();
        let mut u = sample();
        assert!((c.estimate(&u).unwrap() - 2.74).abs() < 1e-10);
        u.input = 101;
        assert!((c.estimate(&u).unwrap() - 4.5).abs() < 1e-10);
        u.source = "live".into();
        assert!((c.estimate(&u).unwrap() - 2.75).abs() < 1e-10); // aggregate is explicitly approximate
        u.model = "unknown".into();
        assert!(c.estimate(&u).is_none());
        u.model = "gpt-test".into();
        u.usd = Some(0.);
        assert!(c.estimate(&u).is_none());
        u.usd = None;
        u.cache_read = Some(200);
        assert!(c.estimate(&u).is_none());
    }
    #[test]
    fn rejects_invalid_prices_and_unsupported_pricing_rules() {
        assert!(
            Catalog::parse(
                &json!({"data":[{"id":"openai/x","pricing":{"prompt":"NaN","completion":"1"}}]})
            )
            .is_none()
        );
        let mut c = prices();
        c.prices.get_mut("openai/gpt-test").unwrap()["overrides"] =
            json!([{"utc_start":"12:00","prompt":"0"}]);
        assert!(c.estimate(&sample()).is_none());
    }
    #[tokio::test]
    async fn native_history_deduplicates_checks_ownership_and_rejects_partial_logs() {
        let d = tempfile::tempdir().unwrap();
        let app = crate::tests::app(d.path());
        let mut r = crate::engine::create(
            &app,
            1,
            crate::presets::all().remove(0),
            "history".into(),
            None,
        )
        .await
        .unwrap();
        let sid = crate::model::id();
        r.sessions.insert(
            "author".into(),
            crate::model::NativeSession {
                runtime: "codex".into(),
                native_id: sid.clone(),
                model: "gpt-test".into(),
                turns: 1,
            },
        );
        let path = d.path().join(format!("rollout-{sid}.jsonl"));
        let header = json!({"type":"session_meta","payload":{"id":sid,"cwd":r.workspace}});
        let context = json!({"type":"turn_context","payload":{"model":"gpt-test","turn_id":"t"}});
        let token = json!({"type":"event_msg","timestamp":"2026-10-31T10:00:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20},"last_token_usage":{"input_tokens":100,"output_tokens":20,"cached_input_tokens":40,"cache_write_input_tokens":10}}}});
        let body = format!("{header}\n{context}\n{token}\n{token}\n");
        std::fs::write(&path, &body).unwrap();
        let files = vec![path.clone()];
        let (rows, missed) = history(&r, &[], &files, &mut 0);
        assert_eq!(missed, 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].input, 100);
        assert_eq!(rows[0].cache_write, Some(10));
        r.workspace = "other".into();
        assert_eq!(history(&r, &[], &files, &mut 0).1, 1);
        r.workspace = header["payload"]["cwd"].as_str().unwrap().into();
        std::fs::write(
            &path,
            format!("{body}{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\"}}}}\n"),
        )
        .unwrap();
        assert!(history(&r, &[], &files, &mut 0).0.is_empty());
        assert_eq!(history(&r, &[], &files, &mut (128 * 1024 * 1024)).1, 1);
    }
}
