//! Atlassian TWG runs as the current NODO user, with its own persistent OAuth store.
use crate::{App, agents, model::*};
use anyhow::{Context, Result, ensure};
use std::{path::Path, process::Stdio};
use tokio::io::AsyncReadExt;

pub fn arguments(command: &str) -> Result<Vec<String>> {
    ensure!(command.len() <= 64 * 1024, "Comando TWG troppo grande");
    let args: Vec<String> = serde_json::from_str(command)
        .context("Gli argomenti TWG devono essere una lista JSON di stringhe")?;
    ensure!(
        !args.is_empty() && args.len() <= 128,
        "Argomenti TWG mancanti o troppi"
    );
    ensure!(
        [
            "jira",
            "confluence",
            "bitbucket",
            "bb",
            "jsm",
            "assets",
            "loom",
            "trello",
            "rovo",
            "goals",
            "projects",
            "teams",
            "people",
            "work",
            "docs",
            "focus-areas",
            "artifacts",
            "search-code",
            "user",
            "user-search",
            "collaborators",
            "resolve",
            "videos",
            "meetings",
            "spaces",
            "pull-requests",
            "prs",
            "recently-viewed",
            "work-tree",
            "workitem-tree",
            "pr-tree",
            "focus-areas-tree",
            "context",
            "subgraph",
            "help",
            "talent",
            "csm",
            "insights",
            "responsibility"
        ]
        .contains(&args[0].as_str()),
        "Seleziona un comando di prodotto TWG; login e setup si eseguono dal terminale personale"
    );
    for arg in &args {
        ensure!(
            !arg.contains('\0') && arg.len() <= 32 * 1024,
            "Argomento TWG non valido"
        );
        ensure!(
            ![
                "--token",
                "--password",
                "--config-dir",
                "--profile",
                "-p",
                "--env",
                "--auth-storage-key"
            ]
            .iter()
            .any(|flag| arg == flag || arg.starts_with(&format!("{flag}="))),
            "Credenziali e profili TWG si configurano nel terminale personale"
        );
    }
    Ok(args)
}

pub(crate) async fn run(
    app: &App,
    uid: i64,
    cwd: &Path,
    args: &[String],
    timeout: u64,
) -> Result<String> {
    run_binary(app, uid, cwd, args, timeout, "twg").await
}

async fn run_binary(
    app: &App,
    uid: i64,
    cwd: &Path,
    args: &[String],
    timeout: u64,
    binary: &str,
) -> Result<String> {
    let settings = app.store.settings(uid)?;
    let home = agents::user_home(app, uid)?;
    let mut cmd = agents::command(app, uid, binary, cwd, &settings)?;
    cmd.args(args)
        .env("TWG_CONFIG_DIR", home.join(".config/twg"))
        .env("TWG_AGENT_DEFAULTS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .context("TWG CLI non disponibile: ricostruisci l'immagine del container")?;
    let _group = agents::ProcessGroup(child.id().context("PID TWG mancante")?);
    let mut stdout = child.stdout.take().context("stdout TWG mancante")?;
    let mut stderr = child.stderr.take().context("stderr TWG mancante")?;
    const LIMIT: u64 = 2 * 1024 * 1024;
    let output = async {
        async fn read_bounded(reader: impl tokio::io::AsyncRead + Unpin) -> Result<Vec<u8>> {
            let mut data = Vec::new();
            reader.take(LIMIT + 1).read_to_end(&mut data).await?;
            ensure!(
                data.len() <= LIMIT as usize,
                "Risposta TWG troppo grande: limita i risultati del comando"
            );
            Ok(data)
        }
        let (out, _err) = tokio::try_join!(read_bounded(&mut stdout), read_bounded(&mut stderr))?;
        let status = child.wait().await?;
        // Never return raw authentication diagnostics or stderr, which may contain secrets.
        ensure!(
            status.success(),
            "TWG non ha completato il comando (codice {:?}). Verifica login, permessi, sintassi e possibili effetti prima di riprovare.",
            status.code()
        );
        Ok::<_, anyhow::Error>(agents::redact(&String::from_utf8_lossy(&out), &settings))
    };
    tokio::time::timeout(std::time::Duration::from_secs(timeout), output)
        .await
        .context("Tempo massimo TWG superato; verifica eventuali effetti prima di riprovare")?
}

pub async fn execute(app: &App, uid: i64, run: &Run, node: &Node) -> Result<AgentResult> {
    ensure!(node.operation == "twg", "Scegli l'operazione Comando TWG");
    let args = arguments(&node.command)?;
    let summary = self::run(app, uid, Path::new(&run.workspace), &args, 120).await?;
    Ok(AgentResult {
        status: "completed".into(),
        summary,
        decision: "pass".into(),
        ..Default::default()
    })
}

pub async fn status(app: &App, uid: i64) -> serde_json::Value {
    let home = match agents::user_home(app, uid) {
        Ok(home) => home,
        Err(_) => return serde_json::json!({"available":false,"connected":false}),
    };
    let version = run(app, uid, &home, &["--version".into()], 15).await;
    let connected = if version.is_ok() {
        run(app, uid, &home, &["doctor".into()], 45).await.is_ok()
    } else {
        false
    };
    serde_json::json!({"available":version.is_ok(), "version":version.unwrap_or_default().trim(), "connected":connected})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twg_accepts_products_without_shell_and_rejects_credentials_and_setup() {
        for args in [
            r#"["jira","workitem","get","ABC-1"]"#,
            r#"["confluence","content","get","123"]"#,
            r#"["bitbucket","pr","get","42"]"#,
            r#"["teams","query"]"#,
        ] {
            assert!(arguments(args).is_ok());
        }
        for args in [
            "jira get ABC-1",
            "[]",
            r#"["login"]"#,
            r#"["setup"]"#,
            r#"["jira","--token=secret"]"#,
            r#"["jira","--config-dir","/data/users/2"]"#,
        ] {
            assert!(arguments(args).is_err(), "{args}");
        }
        let a = arguments(r#"["jira","workitem","get","$(touch /tmp/pwned)"]"#).unwrap();
        assert_eq!(a[3], "$(touch /tmp/pwned)");
    }
    #[test]
    fn twg_requires_approval_for_every_operation() {
        let mut node = Node {
            provider: "atlassian".into(),
            operation: "twg".into(),
            command: r#"["jira","workitem","get","ABC-1"]"#.into(),
            ..Default::default()
        };
        assert!(crate::connectors::is_write(&node));
        node.operation = "read_ticket".into();
        assert!(crate::connectors::is_write(&node));
    }
    #[tokio::test]
    async fn twg_runner_isolates_home_and_rejects_partial_results() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let cli = dir.path().join("fake-twg");
        std::fs::write(
            &cli,
            "#!/bin/sh\nprintf '%s\\n%s\\n' \"$HOME\" \"$TWG_CONFIG_DIR\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        for uid in [1, 2] {
            let out = run_binary(&app, uid, dir.path(), &[], 5, cli.to_str().unwrap())
                .await
                .unwrap();
            let home = dir.path().join(format!("users/{uid}"));
            assert_eq!(
                out,
                format!(
                    "{}\n{}\n",
                    home.display(),
                    home.join(".config/twg").display()
                )
            );
        }
        std::fs::write(&cli, "#!/bin/sh\necho partial\necho secret >&2\nexit 3\n").unwrap();
        let err = run_binary(&app, 1, dir.path(), &[], 5, cli.to_str().unwrap())
            .await
            .unwrap_err()
            .to_string();
        assert!(!err.contains("partial") && !err.contains("secret"));
        std::fs::write(&cli, "#!/bin/sh\nsleep 10\n").unwrap();
        assert!(
            run_binary(&app, 1, dir.path(), &[], 1, cli.to_str().unwrap())
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn twg_workflow_pauses_before_execution_and_digest_tracks_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let mut workflow = crate::presets::all().remove(0);
        let source = workflow
            .nodes
            .iter()
            .find(|n| n.kind == "Source")
            .unwrap()
            .clone();
        let mut node = Node {
            id: "atlassian-test".into(),
            kind: "Connection".into(),
            provider: "atlassian".into(),
            operation: "twg".into(),
            command: r#"["jira","workitem","get","ABC-1"]"#.into(),
            ..Default::default()
        };
        workflow.nodes = vec![source.clone(), node.clone()];
        workflow.edges = vec![Edge {
            from: source.id,
            to: node.id.clone(),
            kind: "context".into(),
            outcome: String::new(),
        }];
        workflow.entry = node.id.clone();
        let run = crate::engine::create(&app, 1, workflow, String::new(), None)
            .await
            .unwrap();
        let settings = Settings::default();
        let digest = crate::connectors::effect_digest(&node, &run, &settings);
        node.command = r#"["jira","workitem","delete","ABC-1"]"#.into();
        assert_ne!(
            digest,
            crate::connectors::effect_digest(&node, &run, &settings)
        );
        crate::engine::launch(app.clone(), 1, run.id.clone()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let current = app.store.run(1, &run.id).unwrap();
                if let Some(pending) = current.pending {
                    assert_eq!(pending.kind, "effect");
                    assert!(pending.questions[0].contains("ABC-1"));
                    break;
                }
                assert_ne!(current.status, "failed", "{:?}", current.error);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
    }
}
