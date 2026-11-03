//! Atlassian TWG runs as the current NODO user, with its own persistent OAuth store.
use crate::{App, agents, model::*};
use anyhow::{Context, Result, ensure};
use std::{path::Path, process::Stdio};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;

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
    run_configured(app, uid, cwd, args, timeout, binary, None).await
}
async fn run_configured(app: &App, uid: i64, cwd: &Path, args: &[String], timeout: u64, binary: &str, draft: Option<&ConnectionConfig>) -> Result<String> {
    let settings = app.store.settings(uid)?;
    let home = agents::user_home(app, uid)?;
    let mut cmd = agents::command(app, uid, binary, cwd, &settings)?;
    let config = draft.or_else(|| settings.connections.get("atlassian"));
    configure_command(&mut cmd, config)?;
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

pub fn site(config: &ConnectionConfig) -> Result<Option<String>> {
    if config.base_url.trim().is_empty() { return Ok(None); }
    let url=reqwest::Url::parse(config.base_url.trim()).context("URL sito Atlassian non valido")?;
    let host=url.host_str().unwrap_or_default();
    ensure!(url.scheme()=="https" && host.ends_with(".atlassian.net") && url.username().is_empty() && url.password().is_none() && url.query().is_none() && url.fragment().is_none() && url.port().is_none() && ["", "/"].contains(&url.path()), "Usa l’URL HTTPS del sito *.atlassian.net, senza percorsi o credenziali");
    Ok(Some(host.to_string()))
}
pub fn validate_config(config: &ConnectionConfig) -> Result<()> {
    site(config)?;
    ensure!(config.token.len()<=4096 && !config.token.chars().any(char::is_whitespace), "Token Bitbucket non valido");
    ensure!(config.username.len()<=254 && (config.username.is_empty() || (config.username.contains('@') && !config.username.chars().any(char::is_whitespace))), "Email Atlassian non valida");
    ensure!(config.repository.is_empty() || (config.repository.split('/').count()==2 && config.repository.split('/').all(|s|!s.is_empty() && s.len()<=100 && s.chars().all(|c|c.is_ascii_alphanumeric() || "-_.".contains(c)))), "Repository Bitbucket deve essere workspace/repository");
    ensure!(config.token.is_empty() || !config.username.is_empty(), "Inserisci l’email Atlassian per il token Bitbucket");
    Ok(())
}
fn configure_command(command: &mut tokio::process::Command, config: Option<&ConnectionConfig>) -> Result<()> {
    for name in ["TWG_TOKEN", "TWG_USER", "TWG_PROFILE", "TWG_SITE", "TWG_DOMAIN", "TWG_BBC_TOKEN", "TWG_BBC_USER"] { command.env_remove(name); }
    if let Some(c)=config {
        if let Some(site)=site(c)? { command.env("TWG_SITE",site); }
        if !c.token.is_empty() { command.env("TWG_BBC_TOKEN", &c.token).env("TWG_BBC_USER", &c.username); }
    }
    Ok(())
}
pub async fn check(app: &App, uid: i64, mut config: ConnectionConfig) -> Result<serde_json::Value> {
    let settings=app.store.settings(uid)?;
    if config.token.is_empty() { config.token=settings.connections.get("atlassian").filter(|c|c.username==config.username).map(|c|c.token.clone()).unwrap_or_default(); }
    if config.token=="__CLEAR__" { config.token.clear(); }
    validate_config(&config)?;
    let cwd=agents::user_home(app,uid)?;
    let oauth=run_configured(app,uid,&cwd,&["user".into(),"get".into(),"-o".into(),"json".into()],45,"twg",Some(&config)).await.is_ok();
    let bitbucket=if config.token.is_empty() { None } else {
        let args=if config.repository.is_empty(){vec!["bitbucket","workspace","query","-o","json"].into_iter().map(String::from).collect()}else{vec!["bitbucket".into(),"repo".into(),"get".into(),"--repo".into(),config.repository.clone(),"-o".into(),"json".into()]};
        Some(run_configured(app,uid,&cwd,&args,45,"twg",Some(&config)).await.is_ok())
    };
    Ok(serde_json::json!({"oauth_connected":oauth,"bitbucket_connected":bitbucket,"site":config.base_url,"repository":config.repository}))
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LoginState { pub state: String, pub verification_url: Option<String> }
static LOGINS: tokio::sync::Mutex<BTreeMap<String,LoginState>>=tokio::sync::Mutex::const_new(BTreeMap::new());
fn login_key(app: &App, uid:i64)->String{format!("{}:{uid}",app.data.display())}
pub async fn login_status(app:&App,uid:i64)->LoginState{LOGINS.lock().await.get(&login_key(app,uid)).cloned().unwrap_or_default()}
pub async fn begin_login(app:&App,uid:i64)->Result<LoginState>{
    let settings=app.store.settings(uid)?;
    let config=settings.connections.get("atlassian").cloned().unwrap_or_default();
    let host=site(&config)?.context("Salva prima l’URL del sito Atlassian")?;
    let key=login_key(app,uid);
    let mut states=LOGINS.lock().await;
    if let Some(state)=states.get(&key) && ["starting","waiting"].contains(&state.state.as_str()){return Ok(state.clone());}
    let home=agents::user_home(app,uid)?;
    // script allocates the terminal required by the CLI. The validated host has no shell metacharacters.
    ensure!(host.chars().all(|c|c.is_ascii_alphanumeric() || "-.".contains(c)),"Sito non valido");
    let mut command=agents::command(app,uid,"script",&home,&settings)?;
    #[cfg(target_os="linux")]
    command.args(["-q","-e","-c",&format!("twg login --site {host} --force"),"/dev/null"]);
    #[cfg(not(target_os="linux"))]
    command.args(["-q","/dev/null","twg","login","--site",&host,"--force"]);
    command.env("TWG_CONFIG_DIR",home.join(".config/twg")).env("TWG_BACKGROUND_UPDATE_CHECK","0").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
    let mut child=command.spawn().context("Impossibile avviare il login Atlassian")?;
    let state=LoginState{state:"starting".into(),verification_url:None};states.insert(key.clone(),state.clone());drop(states);
    tokio::spawn(async move {
        let group=child.id().map(agents::ProcessGroup);
        let mut output=child.stdout.take().unwrap();let mut input=child.stdin.take().unwrap();
        let result=tokio::time::timeout(std::time::Duration::from_secs(600),async {
            let mut bytes=Vec::new();let mut buffer=[0u8;4096];let mut answered=false;
            loop {
                let n=output.read(&mut buffer).await?;if n==0{break;}bytes.extend_from_slice(&buffer[..n]);ensure!(bytes.len()<256*1024,"Risposta login troppo grande");
                let text=String::from_utf8_lossy(&bytes);
                if !answered && text.contains("Open in browser?"){input.write_all(b"n\r").await?;answered=true;}
                const PREFIX:&str="https://auth.atlassian.com/oauth/activate?user_code=";
                if let Some(start)=text.find(PREFIX){let tail=&text[start+PREFIX.len()..];let code:String=tail.chars().take_while(|c|c.is_ascii_alphanumeric()||*c=='-').collect();if !code.is_empty(){LOGINS.lock().await.insert(key.clone(),LoginState{state:"waiting".into(),verification_url:Some(format!("{PREFIX}{code}"))});}}
            }
            Ok::<_,anyhow::Error>(child.wait().await?.success())
        }).await;
        drop(group);
        LOGINS.lock().await.insert(key,LoginState{state:if matches!(result,Ok(Ok(true))){"connected"}else{"failed"}.into(),verification_url:None});
    });
    Ok(state)
}
pub async fn execute(app: &App, uid: i64, run: &Run, node: &Node) -> Result<AgentResult> {
    ensure!(node.operation == "twg", "Scegli l'operazione Comando TWG");
    let mut args = arguments(&node.command)?;
    if !node.repository.trim().is_empty() {
        ensure!(["bitbucket", "bb"].contains(&args[0].as_str()), "Il repository del nodo richiede un comando Bitbucket");
        ensure!(!args.iter().any(|a| a == "--repo" || a.starts_with("--repo=")), "Imposta il repository nel nodo oppure negli argomenti TWG");
        let config = ConnectionConfig { repository: node.repository.trim().into(), ..Default::default() };
        validate_config(&config)?;
        args.extend(["--repo".into(), config.repository]);
    }
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
    #[tokio::test]
    async fn browser_login_is_personal_and_returns_only_the_verification_link() {
        use std::os::unix::fs::PermissionsExt;
        let dir=tempfile::tempdir().unwrap();let app=crate::tests::app(dir.path());
        let bin=crate::distribution::tools(dir.path());std::fs::create_dir_all(&bin).unwrap();
        let cli=bin.join("twg");std::fs::write(&cli,"#!/bin/sh\necho 'Visit: https://auth.atlassian.com/oauth/activate?user_code=TEST-1234'\necho 'Open in browser?'\nread answer\nsleep 1\nexit 0\n").unwrap();std::fs::set_permissions(&cli,std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut settings=Settings::default();settings.connections.insert("atlassian".into(),ConnectionConfig{base_url:"https://demo.atlassian.net".into(),..Default::default()});app.store.save_settings(1,&settings).unwrap();
        assert_eq!(begin_login(&app,1).await.unwrap().state,"starting");
        tokio::time::timeout(std::time::Duration::from_secs(10),async{
            loop {let state=login_status(&app,1).await;
                if state.state=="waiting"{assert_eq!(state.verification_url.as_deref(),Some("https://auth.atlassian.com/oauth/activate?user_code=TEST-1234"));assert_eq!(login_status(&app,2).await.state,"");}
                if state.state=="connected"{assert!(state.verification_url.is_none());break;}
                assert_ne!(state.state,"failed");tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
    }
    #[test]
    fn personal_settings_validate_targets_and_inject_secrets_only_into_twg() {
        let c=ConnectionConfig{base_url:"https://demo.atlassian.net".into(),username:"user@example.test".into(),token:"private-bitbucket-secret".into(),repository:"workspace/repo".into(),..Default::default()};
        validate_config(&c).unwrap();
        for url in ["http://demo.atlassian.net","https://evil.example", "https://user@demo.atlassian.net", "https://demo.atlassian.net/path", "https://demo.atlassian.net?x=1"] {
            let mut invalid=c.clone();invalid.base_url=url.into();assert!(validate_config(&invalid).is_err());
        }
        let mut command=tokio::process::Command::new("twg");configure_command(&mut command,Some(&c)).unwrap();
        let env:std::collections::BTreeMap<_,_>=command.as_std().get_envs().map(|(k,v)|(k.to_string_lossy().to_string(),v.map(|v|v.to_string_lossy().to_string()))).collect();
        assert_eq!(env["TWG_BBC_TOKEN"].as_deref(),Some(c.token.as_str()));
        assert_eq!(env["TWG_BBC_USER"].as_deref(),Some(c.username.as_str()));
        assert_eq!(env["TWG_SITE"].as_deref(),Some("demo.atlassian.net"));
        assert_eq!(env["TWG_TOKEN"],None);
        let mut settings=Settings::default();settings.connections.insert("atlassian".into(),c.clone());
        let basic=base64::Engine::encode(&base64::engine::general_purpose::STANDARD,format!("{}:{}",c.username,c.token));
        let redacted=agents::redact(&format!("{} {}",c.token,basic),&settings);
        assert!(!redacted.contains(&c.token));assert!(!redacted.contains(&basic));
    }
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
