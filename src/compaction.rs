use crate::{
    App, agents,
    auth::{ApiResult, bad},
    model::*,
};
use anyhow::{Context, Result, bail, ensure};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::Path as FsPath, process::Stdio};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// A restart must never turn an unfinished compaction into an apparent success.
pub fn recover(store: &crate::store::Store) -> Result<()> {
    let mut pending = std::collections::HashMap::new();
    {
        let db = store.db.lock().unwrap();
        let mut q = db.prepare("SELECT user_id,json FROM events WHERE json_extract(json,'$.kind')='compaction' ORDER BY seq")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (uid, raw) = row?;
            let event: Event = serde_json::from_str(&raw)?;
            let Ok(data) = serde_json::from_str::<Value>(&event.text) else {
                continue;
            };
            let Some(operation) = data["operation"].as_str() else {
                continue;
            };
            let key = (uid, operation.to_owned());
            if data["phase"] == "start" {
                pending.insert(key, (event, data));
            } else {
                pending.remove(&key);
            }
        }
    }
    for ((uid, _), (event, mut data)) in pending {
        data["phase"] = json!("error");
        data["message"] = json!(
            "Compattazione interrotta dal riavvio del server. La sessione è conservata; verifica l’esito prima di riprovare."
        );
        store.event(
            uid,
            &event.run,
            "compaction",
            &event.node,
            &data.to_string(),
        )?;
    }
    Ok(())
}

/// Retired Claude token-window overrides must never be restored by stale clients.
pub fn normalize(s: &mut Settings) {
    let c = s
        .compaction
        .entry("claude".into())
        .or_insert_with(|| CompactionConfig::for_runtime("claude"));
    c.context_window_tokens = 0;
    c.threshold_tokens = 0;
}

pub fn validate(s: &Settings) -> Result<()> {
    ensure!(
        s.workflow_parallelism > 0,
        "Il numero di workflow paralleli deve essere maggiore di zero"
    );
    ensure!(s.task_timeout_secs > 0, "Il timeout deve essere maggiore di zero");
    for (rt, c) in &s.compaction {
        ensure!(
            ["claude", "codex", "pi"].contains(&rt.as_str()),
            "Runtime compattazione non valido"
        );
        ensure!(
            c.context_window_tokens <= 10_000_000
                && c.threshold_tokens <= 10_000_000
                && c.trigger_percent <= 100,
            "Soglie di compattazione non valide"
        );
        if rt == "claude" {
            ensure!(
                (0..=100).contains(&c.trigger_percent),
                "Soglia Claude: da 0 a 100 percento (0 = gestione nativa)"
            );
        }
        if rt == "codex" {
            ensure!(
                c.threshold_tokens == 0 || c.threshold_tokens >= 1024,
                "Soglia Codex: 0 oppure almeno 1024 token"
            );
            ensure!(
                c.context_window_tokens == 0 || c.context_window_tokens >= 4096,
                "Finestra Codex troppo piccola"
            );
            ensure!(
                c.context_window_tokens == 0 || c.threshold_tokens < c.context_window_tokens,
                "La soglia Codex deve essere inferiore alla finestra di contesto"
            );
        }
        if rt == "pi" {
            ensure!(
                (1024..=1_000_000).contains(&c.reserve_tokens)
                    && (1024..=1_000_000).contains(&c.keep_recent_tokens),
                "Budget Pi: da 1024 a 1000000 token"
            );
        }
    }
    Ok(())
}
/// Keep unrelated native Pi settings and credentials intact.
pub fn configure(
    app: &App,
    uid: i64,
    rt: &str,
    s: &Settings,
    cmd: &mut tokio::process::Command,
) -> Result<()> {
    let c = s
        .compaction
        .get(rt)
        .cloned()
        .unwrap_or_else(|| CompactionConfig::for_runtime(rt));
    match rt {
        "claude" => {
            // The CLI resolves the current model's native window. Ignore legacy app/host
            // window overrides and override saved autoCompactWindow settings for this launch.
            cmd.env_remove("CLAUDE_CODE_AUTO_COMPACT_WINDOW");
            cmd.args(["--autocompact", "auto"]);
            if c.trigger_percent == 0 {
                cmd.env_remove("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE");
            } else {
                cmd.env(
                    "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE",
                    c.trigger_percent.to_string(),
                );
            }
        }
        "codex" => {
            if c.context_window_tokens > 0 {
                cmd.args([
                    "-c",
                    &format!("model_context_window={}", c.context_window_tokens),
                ]);
            }
            if c.threshold_tokens > 0 {
                cmd.args([
                    "-c",
                    &format!("model_auto_compact_token_limit={}", c.threshold_tokens),
                ]);
            }
        }
        "pi" => {
            let home = agents::user_home(app, uid)?;
            let path = home.join(".pi/agent/settings.json");
            ensure!(!path.is_symlink(), "Configurazione Pi non valida");
            let mut native = if path.exists() {
                ensure!(
                    std::fs::metadata(&path)?.len() <= 1024 * 1024,
                    "Configurazione Pi troppo grande"
                );
                use std::io::Read;
                use std::os::unix::fs::OpenOptionsExt;
                let mut file = std::fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&path)?;
                ensure!(
                    file.metadata()?.len() <= 1024 * 1024,
                    "Configurazione Pi troppo grande"
                );
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                serde_json::from_slice::<Value>(&bytes)?
            } else {
                json!({})
            };
            ensure!(native.is_object(), "Configurazione Pi non valida");
            if !native["compaction"].is_object() {
                native["compaction"] = json!({});
            }
            native["compaction"]["enabled"] = json!(true);
            native["compaction"]["reserveTokens"] = json!(c.reserve_tokens);
            native["compaction"]["keepRecentTokens"] = json!(c.keep_recent_tokens);
            let temp = home
                .join(".pi/agent")
                .join(format!("settings-{}.tmp", id()));
            std::fs::write(&temp, serde_json::to_vec_pretty(&native)?)?;
            crate::store::private(&temp, false)?;
            agents::own(app, uid, &temp)?;
            std::fs::rename(temp, path)?;
        }
        _ => bail!("Runtime sconosciuto"),
    }
    Ok(())
}
/// Only metrics and lifecycle: never persist summaries or raw context in the activity log.
pub fn observe(rt: &str, v: &Value) -> Option<Value> {
    let ty = v["type"].as_str().unwrap_or("");
    let mut event = json!({"runtime":rt,"mode":"automatic"});
    if rt == "claude" && ty == "system" && v["subtype"] == "status" && v["status"] == "compacting" {
        event["phase"] = json!("start");
    } else if rt == "claude" && ty == "system" && v["subtype"] == "compact_boundary" {
        event["phase"] = json!("end");
        event["mode"] = v["compact_metadata"]["trigger"]
            .as_str()
            .map(|s| json!(s))
            .unwrap_or(json!("automatic"));
        event["tokens_before"] = v["compact_metadata"]["pre_tokens"].clone();
        event["tokens_after_estimate"] = v["compact_metadata"]["post_tokens"].clone();
    } else if rt == "pi"
        && [
            "compaction_start",
            "compaction_end",
            "auto_compaction_start",
            "auto_compaction_end",
        ]
        .contains(&ty)
    {
        event["phase"] = json!(if ty.ends_with("start") {
            "start"
        } else if v["aborted"] == true || v["errorMessage"].is_string() {
            "error"
        } else {
            "end"
        });
        event["mode"] = v["reason"].clone();
        event["tokens_before"] = v["result"]["tokensBefore"].clone();
        event["tokens_after_estimate"] = v["result"]["estimatedTokensAfter"].clone();
        if let Some(message) = v["errorMessage"].as_str() {
            event["message"] = json!(message);
        }
    } else if rt == "codex"
        && (ty == "thread.compacted"
            || ty == "context_compacted"
            || (["item.started", "item.completed"].contains(&ty)
                && ["context_compaction", "contextCompaction"]
                    .contains(&v["item"]["type"].as_str().unwrap_or(""))))
    {
        event["phase"] = json!(if ty == "item.started" { "start" } else { "end" });
    } else {
        return None;
    }
    Some(event)
}
enum Progress<'a> {
    Started,
    Completed,
    Failed(&'a str),
}
fn event(
    app: &App,
    uid: i64,
    run: &Run,
    binding: &str,
    session: &NativeSession,
    operation: &str,
    progress: Progress<'_>,
) {
    let (phase, message) = match progress {
        Progress::Started => ("start", ""),
        Progress::Completed => ("end", ""),
        Progress::Failed(message) => ("error", message),
    };
    let s = app.store.settings(uid).unwrap_or_default();
    let v = json!({"runtime":session.runtime,"session":session.native_id,"binding":binding,"operation":operation,"mode":"manual","phase":phase,"message":message});
    agents::emit(
        app,
        uid,
        &run.id,
        "compaction",
        &run.current,
        &agents::redact(&v.to_string(), &s),
    );
}
#[derive(Deserialize)]
pub struct CompactRequest {
    binding: String,
}
struct Reservation {
    app: App,
    run: String,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.app.busy.lock().unwrap().remove(&self.run);
    }
}
pub async fn start(
    State(app): State<App>,
    user: User,
    Path(rid): Path<String>,
    Json(b): Json<CompactRequest>,
) -> ApiResult<Json<Value>> {
    let lifecycle = app.lifecycle.clone();
    let _guard = lifecycle.write().await;
    let run = app.store.run(user.id, &rid)?;
    let session = run
        .sessions
        .get(&b.binding)
        .cloned()
        .filter(|s| !s.native_id.is_empty())
        .ok_or_else(|| bad("Sessione nativa non disponibile"))?;
    if ["running", "starting", "queued", "waiting_child"].contains(&run.status.as_str()) {
        return Err(bad(
            "Metti in pausa il workflow e attendi la fine del turno prima di compattare",
        ));
    }
    {
        let mut busy = app.busy.lock().unwrap();
        if !busy.insert(rid.clone()) {
            return Err(bad("Esecuzione o compattazione già attiva"));
        }
    }
    let reservation = Reservation {
        app: app.clone(),
        run: rid,
    };
    let operation = id();
    event(
        &app,
        user.id,
        &run,
        &b.binding,
        &session,
        &operation,
        Progress::Started,
    );
    let result = json!({"ok":true,"operation":operation});
    tokio::spawn(async move {
        let _reservation = reservation;
        let result = native(&app, user.id, &run, &session).await;
        match result {
            Ok(()) => event(
                &app,
                user.id,
                &run,
                &b.binding,
                &session,
                &operation,
                Progress::Completed,
            ),
            Err(e) => event(
                &app,
                user.id,
                &run,
                &b.binding,
                &session,
                &operation,
                Progress::Failed(&e.to_string()),
            ),
        }
    });
    Ok(Json(result))
}
async fn send(stdin: &mut tokio::process::ChildStdin, v: Value) -> Result<()> {
    stdin.write_all(format!("{v}\n").as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
}
/// Local slash-command errors can have a zero exit code and is_error=false.
/// Keep the CLI's real diagnostic instead of replacing it with a missing-boundary error.
fn claude_compaction_error(v: &Value) -> Option<&str> {
    let text = v["result"].as_str()?.trim();
    if v["is_error"] == true
        || [
            "Error during compaction:",
            "Compaction failed:",
            "Failed to compact",
            "Not enough messages to compact",
            "Nothing to compact",
        ]
        .iter()
        .any(|prefix| text.starts_with(prefix))
    {
        Some(text)
    } else {
        None
    }
}

async fn native(app: &App, uid: i64, run: &Run, session: &NativeSession) -> Result<()> {
    let mut settings = app.store.settings(uid)?;
    if session.runtime=="pi" {
        let selected=crate::pi_providers::effective(&settings,&session.provider,&session.model,&session.base_url)?;
        settings.agents.insert("pi".into(),selected);
    }
    let cfg = settings
        .agents
        .get(&session.runtime)
        .cloned()
        .unwrap_or_default();
    let rt = session.runtime.as_str();
    let mut cmd = agents::command(app, uid, rt, FsPath::new(&run.workspace), &settings)?;
    configure(app, uid, rt, &settings, &mut cmd)?;
    if rt == "pi" {crate::pi_local::configure(app,uid,&settings,&session.model,&mut cmd)?;}
    // CODEX_API_KEY is an exec-only shortcut. App Server requires an explicit login;
    // keep it in memory so a personal API key never overwrites the ChatGPT auth file.
    let codex_api_key = if rt == "codex" {
        cmd.as_std().get_envs().find_map(|(k, v)| {
            if k == "CODEX_API_KEY" {
                v.filter(|s| !s.is_empty())
                    .map(|v| v.to_string_lossy().into_owned())
            } else {
                None
            }
        })
    } else {
        None
    };
    match rt {
        "claude" => {
            cmd.args([
                "--print",
                "--verbose",
                "--output-format",
                "stream-json",
                "--resume",
                &session.native_id,
                "--setting-sources",
                "project",
                "--strict-mcp-config",
                "--tools",
                "",
            ]);
            if !session.model.is_empty() {
                cmd.args(["--model", &session.model]);
            }
        }
        "codex" => {
            if codex_api_key.is_some() {
                cmd.args(["-c", "cli_auth_credentials_store=\"ephemeral\""]);
            }
            cmd.args(["--no-daemon", "app-server"]);
        }
        "pi" => {
            cmd.args([
                "--mode",
                "rpc",
                "--session",
                &session.native_id,
                "--session-dir",
            ])
            .arg(agents::user_home(app, uid)?.join(".pi/agent/sessions"));
            cmd.args(["--approve", "--no-mcp", "--no-tools", "--no-extensions"]);
            if !session.model.is_empty() {
                cmd.args(["--model", &session.model]);
            }
            if !cfg.provider.is_empty() {
                cmd.args(["--provider", &cfg.provider]);
            }
        }
        _ => bail!("Runtime sconosciuto"),
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("CLI di compattazione non disponibile")?;
    let _group = agents::ProcessGroup(child.id().context("PID non disponibile")?);
    // Drain stderr continuously, retaining only a bounded diagnostic suffix.
    let mut stderr = child.stderr.take().unwrap();
    let errors = tokio::spawn(async move {
        let mut kept = Vec::new();
        let mut b = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut b).await {
            if n == 0 {
                break;
            }
            kept.extend_from_slice(&b[..n]);
            if kept.len() > 16384 {
                kept.drain(..kept.len() - 16384);
            }
        }
        kept
    });
    let mut stdin = Some(child.stdin.take().unwrap());
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let task = async {
        if rt == "claude" {
            let mut input = stdin.take().unwrap();
            input.write_all(b"/compact").await?;
            input.shutdown().await?;
            drop(input);
        } else if rt == "pi" {
            send(
                stdin.as_mut().unwrap(),
                json!({"id":"compact","type":"compact"}),
            )
            .await?;
        } else {
            send(stdin.as_mut().unwrap(), json!({"id":0,"method":"initialize","params":{"clientInfo":{"name":"rightcommit","version":env!("CARGO_PKG_VERSION")}}})).await?;
        }
        let mut count = 0;
        let mut boundary = false;
        while let Some(line) = lines.next_line().await? {
            count += line.len();
            ensure!(
                count < 32 * 1024 * 1024,
                "Output compattazione oltre il limite"
            );
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if rt == "codex" {
                if let Some(error) = v.get("error") {
                    bail!(
                        "{}",
                        error["message"]
                            .as_str()
                            .unwrap_or("Errore Codex durante compattazione")
                    );
                }
                match v["id"].as_u64() {
                    Some(0) => {
                        send(
                            stdin.as_mut().unwrap(),
                            json!({"method":"initialized","params":{}}),
                        )
                        .await?;
                        if let Some(key) = &codex_api_key {
                            send(stdin.as_mut().unwrap(), json!({"id":3,"method":"account/login/start","params":{"type":"apiKey","apiKey":key}})).await?;
                        } else {
                            let mut params = json!({"threadId":session.native_id,"cwd":run.workspace,"approvalPolicy":"never","sandbox":"readOnly"});
                            if !session.model.is_empty() {
                                params["model"] = json!(session.model);
                            }
                            send(
                                stdin.as_mut().unwrap(),
                                json!({"id":1,"method":"thread/resume","params":params}),
                            )
                            .await?;
                        }
                    }
                    Some(3) => {
                        let mut params = json!({"threadId":session.native_id,"cwd":run.workspace,"approvalPolicy":"never","sandbox":"readOnly"});
                        if !session.model.is_empty() {
                            params["model"] = json!(session.model);
                        }
                        send(
                            stdin.as_mut().unwrap(),
                            json!({"id":1,"method":"thread/resume","params":params}),
                        )
                        .await?;
                    }
                    Some(1) => {
                        ensure!(
                            v["result"]["thread"]["id"] == session.native_id,
                            "Codex ha restituito una sessione diversa"
                        );
                        send(stdin.as_mut().unwrap(), json!({"id":2,"method":"thread/compact/start","params":{"threadId":session.native_id}})).await?;
                    }
                    _ => {}
                }
                if v["method"] == "error" && v["params"]["willRetry"] != true {
                    bail!(
                        "{}",
                        v["params"]["error"]["message"]
                            .as_str()
                            .unwrap_or("Compattazione Codex fallita")
                    );
                }
                if v["method"] == "item/completed"
                    && v["params"]["threadId"] == session.native_id
                    && v["params"]["item"]["type"] == "contextCompaction"
                {
                    boundary = true;
                }
                if v["method"] == "turn/completed" && v["params"]["threadId"] == session.native_id {
                    ensure!(
                        v["params"]["turn"]["status"] == "completed",
                        "Compattazione Codex fallita: {}",
                        v["params"]["turn"]["error"]
                    );
                    ensure!(boundary, "Codex non ha confermato la compattazione");
                    return Ok(());
                }
            } else if rt == "pi" && v["type"] == "response" && v["id"] == "compact" {
                ensure!(
                    v["success"] == true,
                    "{}",
                    v["error"].as_str().unwrap_or("Compattazione Pi fallita")
                );
                return Ok(());
            } else if rt == "claude" {
                if v["type"] == "system" && v["subtype"] == "compact_boundary" {
                    boundary = true;
                }
                if v["type"] == "result" {
                    if let Some(message) = claude_compaction_error(&v) {
                        bail!("{message}");
                    }
                    ensure!(
                        v["is_error"] != true,
                        "{}",
                        v["result"]
                            .as_str()
                            .unwrap_or("Compattazione Claude fallita")
                    );
                    ensure!(
                        boundary,
                        "Claude non ha confermato la compattazione della sessione"
                    );
                    return Ok(());
                }
            }
        }
        bail!("La CLI è terminata senza confermare la compattazione")
    };
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(settings.task_timeout_secs.max(1)),
        task,
    )
    .await;
    if rt == "claude" && matches!(&outcome, Ok(Ok(()))) {
        let status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await;
        ensure!(
            matches!(status, Ok(Ok(s)) if s.success()),
            "Claude non ha salvato correttamente la compattazione"
        );
    } else {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    unsafe {
        libc::kill(-(_group.0 as i32), libc::SIGKILL);
    }
    let diagnostic = tokio::time::timeout(std::time::Duration::from_secs(1), errors)
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    match outcome {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            let text = String::from_utf8_lossy(&diagnostic);
            let suffix: String = text.chars().take(2000).collect();
            if suffix.is_empty() {
                Err(error)
            } else {
                bail!("{error}: {suffix}")
            }
        }
        Err(_) => bail!("Timeout durante la compattazione; sessione conservata"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_settings_use_claude_native_window_80_percent_and_five_workers() {
        let s: Settings = serde_json::from_value(json!({"task_timeout_secs":600})).unwrap();
        assert_eq!(s.workflow_parallelism, 5);
        assert_eq!(s.compaction["claude"].context_window_tokens, 0);
        assert_eq!(s.compaction["claude"].trigger_percent, 80);
        assert_eq!(s.compaction["codex"].context_window_tokens, 0);
        assert_eq!(s.compaction["codex"].threshold_tokens, 0);
        assert_eq!(s.compaction["pi"].reserve_tokens, 16384);
        validate(&s).unwrap();
        let mut s = s;
        s.workflow_parallelism = 0;
        assert!(validate(&s).is_err());
        s.workflow_parallelism = 128;
        s.task_timeout_secs = 86400;
        validate(&s).unwrap();
        s.task_timeout_secs = 0;
        assert!(validate(&s).is_err());
        s.task_timeout_secs = 7200;
        s.workflow_parallelism = 5;
        s.compaction.get_mut("claude").unwrap().trigger_percent = 101;
        assert!(validate(&s).is_err());
        s.compaction.get_mut("claude").unwrap().trigger_percent = 80;
        normalize(&mut s);
        let c = s.compaction.get_mut("codex").unwrap();
        c.context_window_tokens = 200000;
        c.threshold_tokens = 200000;
        assert!(validate(&s).is_err());
        s.compaction.get_mut("codex").unwrap().threshold_tokens = 160000;
        validate(&s).unwrap();
    }
    #[test]
    fn obsolete_claude_windows_are_cleared_without_changing_other_runtimes() {
        let mut s = Settings::default();
        s.compaction
            .get_mut("claude")
            .unwrap()
            .context_window_tokens = 800000;
        s.compaction.get_mut("claude").unwrap().threshold_tokens = 120000;
        s.compaction.get_mut("claude").unwrap().trigger_percent = 65;
        s.compaction.get_mut("codex").unwrap().threshold_tokens = 160000;
        normalize(&mut s);
        assert_eq!(s.compaction["claude"].context_window_tokens, 0);
        assert_eq!(s.compaction["claude"].threshold_tokens, 0);
        assert_eq!(s.compaction["claude"].trigger_percent, 65);
        assert_eq!(s.compaction["codex"].threshold_tokens, 160000);
        assert_eq!(s.compaction["pi"].reserve_tokens, 16384);
    }
    #[test]
    fn zero_claude_percentage_uses_native_compaction_without_host_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let mut s = Settings::default();
        s.compaction.get_mut("claude").unwrap().trigger_percent = 0;
        s.compaction
            .get_mut("claude")
            .unwrap()
            .context_window_tokens = 800000;
        normalize(&mut s);
        assert_eq!(s.compaction["claude"].trigger_percent, 0);
        validate(&s).unwrap();
        let mut cmd = agents::command(&app, 1, "claude", dir.path(), &s).unwrap();
        cmd.env("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE", "37");
        cmd.env("CLAUDE_CODE_AUTO_COMPACT_WINDOW", "200000");
        configure(&app, 1, "claude", &s, &mut cmd).unwrap();
        assert!(!cmd.as_std().get_envs().any(|(k, v)| v.is_some()
            && (k == "CLAUDE_CODE_AUTO_COMPACT_WINDOW" || k == "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE")));
        let args: Vec<_> = cmd
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|a| a == ["--autocompact", "auto"]));
    }
    #[test]
    fn native_configuration_is_applied_without_exposing_keys_or_overwriting_other_pi_settings() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let mut s = Settings::default();
        s.compaction
            .get_mut("claude")
            .unwrap()
            .context_window_tokens = 200000;
        s.compaction.get_mut("claude").unwrap().trigger_percent = 70;
        let mut cmd = agents::command(&app, 1, "claude", dir.path(), &s).unwrap();
        cmd.env("CLAUDE_CODE_AUTO_COMPACT_WINDOW", "200000");
        configure(&app, 1, "claude", &s, &mut cmd).unwrap();
        let env: std::collections::BTreeMap<_, _> = cmd
            .as_std()
            .get_envs()
            .filter_map(|(k, v)| {
                v.map(|v| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect();
        assert!(!env.contains_key("CLAUDE_CODE_AUTO_COMPACT_WINDOW"));
        let args: Vec<_> = cmd
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|a| a == ["--autocompact", "auto"]));
        assert_eq!(env["CLAUDE_AUTOCOMPACT_PCT_OVERRIDE"], "70");
        let path = agents::user_home(&app, 1)
            .unwrap()
            .join(".pi/agent/settings.json");
        std::fs::write(
            &path,
            r#"{"theme":"dark","compaction":{"enabled":false},"defaultProvider":"openrouter"}"#,
        )
        .unwrap();
        s.compaction.get_mut("pi").unwrap().reserve_tokens = 32000;
        configure(&app, 1, "pi", &s, &mut cmd).unwrap();
        let native: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(native["theme"], "dark");
        assert_eq!(native["compaction"]["enabled"], true);
        assert_eq!(native["compaction"]["reserveTokens"], 32000);
        let mut cmd = agents::command(&app, 1, "codex", dir.path(), &s).unwrap();
        s.compaction.get_mut("codex").unwrap().threshold_tokens = 160000;
        configure(&app, 1, "codex", &s, &mut cmd).unwrap();
        assert!(
            cmd.as_std()
                .get_args()
                .any(|v| v == "model_auto_compact_token_limit=160000")
        );
    }
    #[test]
    fn restart_reports_unfinished_manual_compaction_once_and_keeps_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        app.store
            .event(
                1,
                "run",
                "compaction",
                "task",
                &json!({"operation":"interrupted","phase":"start","runtime":"claude"}).to_string(),
            )
            .unwrap();
        app.store
            .event(
                2,
                "run2",
                "compaction",
                "task",
                &json!({"operation":"done","phase":"start"}).to_string(),
            )
            .unwrap();
        app.store
            .event(
                2,
                "run2",
                "compaction",
                "task",
                &json!({"operation":"done","phase":"end"}).to_string(),
            )
            .unwrap();
        recover(&app.store).unwrap();
        recover(&app.store).unwrap();
        let events = app.store.events(1, "run", 0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            serde_json::from_str::<Value>(&events[1].text).unwrap()["phase"],
            "error"
        );
        assert_eq!(app.store.events(2, "run2", 0).unwrap().len(), 2);
    }
    #[test]
    fn claude_local_compaction_errors_are_not_mistaken_for_success() {
        let v = json!({"type":"result","subtype":"success","is_error":false,"result":"Error during compaction: API Error: Connection refused (ECONNREFUSED)"});
        assert!(
            claude_compaction_error(&v)
                .unwrap()
                .contains("ECONNREFUSED")
        );
        assert_eq!(
            claude_compaction_error(&json!({"is_error":false,"result":""})),
            None
        );
        assert_eq!(
            claude_compaction_error(&json!({"is_error":false,"result":"Compacted"})),
            None
        );
    }
    #[test]
    fn lifecycle_events_include_metrics_but_never_summaries() {
        let event = observe("pi", &json!({"type":"compaction_end","reason":"threshold","result":{"summary":"PRIVATE CONTEXT","tokensBefore":180000,"estimatedTokensAfter":30000}})).unwrap();
        assert_eq!(event["phase"], "end");
        assert_eq!(event["tokens_before"], 180000);
        assert!(!event.to_string().contains("PRIVATE"));
        assert_eq!(
            observe(
                "pi",
                &json!({"type":"compaction_end","errorMessage":"ECONNREFUSED"})
            )
            .unwrap()["phase"],
            "error"
        );
        assert_eq!(observe("claude", &json!({"type":"system","subtype":"compact_boundary","compact_metadata":{"trigger":"auto","pre_tokens":190000}})).unwrap()["tokens_before"], 190000);
        assert_eq!(
            observe(
                "codex",
                &json!({"type":"item.completed","item":{"type":"context_compaction"}})
            )
            .unwrap()["phase"],
            "end"
        );
        assert!(observe("claude", &json!({"type":"assistant","message":"ordinary"})).is_none());
    }
}
