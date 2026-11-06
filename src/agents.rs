use crate::{
    App,
    model::*,
    store::{hash, private},
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

pub(crate) struct ProcessGroup(pub u32);
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-(self.0 as i32), libc::SIGKILL);
        }
    }
}

pub fn user_home(app: &App, uid: i64) -> Result<PathBuf> {
    let users = app.data.join("users");
    std::fs::create_dir_all(&users)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&app.data, std::fs::Permissions::from_mode(0o711))?;
    std::fs::set_permissions(&users, std::fs::Permissions::from_mode(0o711))?;
    let home = users.join(uid.to_string());
    ensure!(!home.is_symlink(), "Home utente non valida");
    std::fs::create_dir_all(&home)?;
    private(&home, true)?;
    own(app, uid, &home)?;
    for d in [
        ".config",
        ".config/twg",
        ".aws",
        ".codex",
        ".claude",
        ".pi",
        ".pi/agent",
        "workspaces",
        "manager",
        "manager/mcp-servers",
        "manager/projects",
        "tmp",
        ".cargo",
    ] {
        let p = home.join(d);
        ensure!(!p.is_symlink(), "Directory runtime non valida");
        std::fs::create_dir_all(&p)?;
        own(app, uid, &p)?;
    }
    Ok(home)
}
pub fn own(app: &App, uid: i64, p: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    if app.isolate {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(p.as_os_str().as_bytes())?;
        ensure!(
            unsafe { libc::lchown(c.as_ptr(), (20000 + uid) as u32, (20000 + uid) as u32) } == 0,
            "chown fallito"
        );
    }
    let _ = (app, uid, p);
    Ok(())
}
pub fn own_tree(app: &App, uid: i64, p: &Path) -> Result<()> {
    // Native installs never change ownership. Avoid a recursive no-op stat walk.
    if !cfg!(target_os = "linux") || !app.isolate { return Ok(()); }
    own(app, uid, p)?;
    if p.is_dir() && !p.is_symlink() {
        for e in std::fs::read_dir(p)? {
            own_tree(app, uid, &e?.path())?;
        }
    }
    Ok(())
}
/// Resolve API-key inheritance at execution time; never persist or publish the admin key.
pub(crate) fn effective_agent_config(app: &App, uid: i64, runtime: &str, config: &AgentConfig) -> Result<AgentConfig> {
    let mut effective = config.clone();
    if uid == 1 || (!config.api_key.is_empty() && !(runtime == "pi" && config.multi_provider)) {return Ok(effective);}
    let admin = app.store.settings(1)?;
    if let Some(shared) = admin.agents.get(runtime).filter(|shared| !shared.api_key.is_empty()) {
        let endpoint = |cfg: &AgentConfig| {
            let url = if cfg.provider == "local" && cfg.base_url.trim().is_empty() {
                std::env::var("PI_LOCAL_BASE_URL").unwrap_or_default()
            } else {cfg.base_url.clone()};
            url.trim().trim_end_matches('/').to_string()
        };
        if runtime != "pi" || (!shared.multi_provider && shared.provider == config.provider
            && endpoint(shared) == endpoint(config)) {
            effective.api_key = shared.api_key.clone();
            if runtime == "pi" {effective.multi_provider = false;}
        }
    }
    Ok(effective)
}
pub(crate) fn redact_for_user(app: &App, uid: i64, text: &str, settings: &Settings) -> String {
    let text = redact(text, settings);
    if uid != 1 && let Ok(admin) = app.store.settings(1) {redact(&text, &admin)} else {text}
}
pub fn command(app: &App, uid: i64, bin: &str, cwd: &Path, settings: &Settings) -> Result<Command> {
    let home = user_home(app, uid)?;
    let mut paths = vec![crate::distribution::tools(&app.data)];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_else(|| "/usr/local/bin:/usr/bin:/bin".into())));
    let search_path = std::env::join_paths(paths)?;
    let mut c = if app.isolate {
        let mut c = Command::new("/usr/bin/setpriv");
        c.args([
            "--reuid",
            &(20000 + uid).to_string(),
            "--regid",
            &(20000 + uid).to_string(),
            "--clear-groups",
            "--no-new-privs",
            bin,
        ]);
        c
    } else {
        Command::new(bin)
    };
    c.env_clear()
        .env(
            "PATH",
            search_path,
        )
        .env("HOME", &home)
        .env("USER", format!("nodo-{uid}"))
        .env("LANG", "C.UTF-8")
        .env("CODEX_HOME", home.join(".codex"))
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("PI_CODING_AGENT_DIR", home.join(".pi/agent"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("TMPDIR", home.join("tmp"))
        .env("CARGO_HOME", home.join(".cargo"))
        .env(
            "RUSTUP_HOME",
            std::env::var("RUSTUP_HOME").unwrap_or_else(|_| {
                format!("{}/.rustup", std::env::var("HOME").unwrap_or_default())
            }),
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "rightcommit")
        .env("GIT_AUTHOR_EMAIL", "rightcommit@localhost")
        .env("GIT_COMMITTER_NAME", "rightcommit")
        .env("GIT_COMMITTER_EMAIL", "rightcommit@localhost")
        .env("SPECKIT_NON_INTERACTIVE", "1")
        .current_dir(cwd)
        .kill_on_drop(true)
        .process_group(0);
    let cfg = effective_agent_config(app, uid, bin, &settings.agents.get(bin).cloned().unwrap_or_default())?;
    if bin == "pi" {crate::pi_providers::validate_auth(&cfg, &home)?;}
    let provider_env = provider_environment_in_home(bin, &cfg, &home, uid);
    ensure!(
        cloud_provider_count(&provider_env) <= 1,
        "Seleziona un solo provider cloud per Claude: AWS, Vertex oppure Foundry"
    );
    if let Some(key) = provider_env.get("ANTHROPIC_API_KEY") {
        ensure!(!key.starts_with("sk-ant-oat"), "Il campo API key richiede una API key Anthropic valida.");
    }
    c.envs(provider_env);
    // Official Spec Kit 1.2.x supports explicit feature-directory resolution.
    // A single living spec is unambiguous even on an isolated nodo/<run> branch.
    let specs = cwd.join("specs");
    if !specs.is_symlink()
        && let Ok(entries) = std::fs::read_dir(&specs)
    {
        let features: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| !p.is_symlink() && p.join("spec.md").is_file())
            .collect();
        if features.len() == 1 {
            c.env("SPECIFY_FEATURE_DIRECTORY", &features[0]);
            if let Some(name) = features[0].file_name() {
                c.env("SPECIFY_FEATURE", name);
            }
        }
    }
    Ok(c)
}
// Only provider variables reach agent processes. HOME/PATH and CLI state stay app-owned.
const CLAUDE_MODEL_ENV: &[&str] = &[
    "ANTHROPIC_MODEL",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_CUSTOM_HEADERS",
    "ANTHROPIC_BETAS",
    "API_TIMEOUT_MS",
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
    "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
];
const AWS_ENV: &[&str] = &[
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_MANTLE",
    "CLAUDE_CODE_USE_ANTHROPIC_AWS",
    "AWS_REGION",
    "AWS_DEFAULT_REGION",
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_PROFILE",
    "AWS_CONFIG_FILE",
    "AWS_SHARED_CREDENTIALS_FILE",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
    "AWS_ROLE_ARN",
    "AWS_ROLE_SESSION_NAME",
    "ANTHROPIC_BEDROCK_BASE_URL",
    "ANTHROPIC_BEDROCK_MANTLE_BASE_URL",
    "ANTHROPIC_BEDROCK_REGION_PREFIX",
    "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
    "ANTHROPIC_AWS_API_KEY",
    "ANTHROPIC_AWS_BASE_URL",
    "ANTHROPIC_AWS_WORKSPACE_ID",
];
const VERTEX_ENV: &[&str] = &[
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_SKIP_VERTEX_AUTH",
    "ANTHROPIC_VERTEX_PROJECT_ID",
    "ANTHROPIC_VERTEX_BASE_URL",
    "CLOUD_ML_REGION",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_CLOUD_PROJECT",
    "GCLOUD_PROJECT",
    "GOOGLE_OAUTH_ACCESS_TOKEN",
];
const FOUNDRY_ENV: &[&str] = &[
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDE_CODE_SKIP_FOUNDRY_AUTH",
    "ANTHROPIC_FOUNDRY_API_KEY",
    "ANTHROPIC_FOUNDRY_AUTH_TOKEN",
    "ANTHROPIC_FOUNDRY_RESOURCE",
    "ANTHROPIC_FOUNDRY_BASE_URL",
    "AZURE_CLIENT_ID",
    "AZURE_TENANT_ID",
    "AZURE_CLIENT_SECRET",
    "AZURE_CLIENT_CERTIFICATE_PATH",
    "AZURE_FEDERATED_TOKEN_FILE",
];
const NETWORK_ENV: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "NODE_EXTRA_CA_CERTS",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CODEX_CA_CERTIFICATE",
];
const HOST_SECRETS: &[&str] = &[
    "CLAUDE_CODE_OAUTH_TOKEN",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_OAUTH_TOKEN",
    "ANTHROPIC_CUSTOM_HEADERS",
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
    "CODEX_ACCESS_TOKEN",
    "OPENROUTER_API_KEY",
    "XAI_API_KEY",
    "DEEPSEEK_API_KEY",
    "PI_LOCAL_API_KEY",
    "ANTHROPIC_API_KEY",
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "ANTHROPIC_AWS_API_KEY",
    "ANTHROPIC_FOUNDRY_API_KEY",
    "ANTHROPIC_FOUNDRY_AUTH_TOKEN",
    "AZURE_CLIENT_SECRET",
    "GOOGLE_OAUTH_ACCESS_TOKEN",
];
fn enabled(value: Option<&String>) -> bool {
    value.is_some_and(|v| ["1", "true", "yes", "on"].contains(&v.to_ascii_lowercase().as_str()))
}
pub(crate) fn uses_cloud_provider(env: &std::collections::BTreeMap<String, String>) -> bool {
    cloud_provider_count(env) > 0
}
fn cloud_provider_count(env: &std::collections::BTreeMap<String, String>) -> usize {
    [
        [
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_MANTLE",
            "CLAUDE_CODE_USE_ANTHROPIC_AWS",
        ]
        .iter()
        .any(|key| enabled(env.get(*key))),
        enabled(env.get("CLAUDE_CODE_USE_VERTEX")),
        enabled(env.get("CLAUDE_CODE_USE_FOUNDRY")),
    ]
    .into_iter()
    .filter(|active| *active)
    .count()
}
pub(crate) fn resolve_provider_environment_for_user(
    uid: i64,
    runtime: &str,
    cfg: &AgentConfig,
    lookup: impl Fn(&str) -> Option<String>,
) -> std::collections::BTreeMap<String, String> {
    let mut env = std::collections::BTreeMap::new();
    // Server provider configuration belongs to the administrator. Other users can
    // receive only an explicit saved API key (their own or the administrator's).
    let host = |name: &str| {
        if uid != 1 && ["claude", "codex"].contains(&runtime) {None}
        else {lookup(name).filter(|value| !value.is_empty())}
    };
    if ["claude", "codex", "pi"].contains(&runtime) {
        for &name in NETWORK_ENV {
            if let Some(value) = lookup(name).filter(|value| !value.is_empty()) {
                env.insert(name.into(), value);
            }
        }
    }
    if runtime == "pi" {
        env.extend(crate::pi_providers::environment(cfg,host));
        return env;
    }
    // A saved personal API key always selects direct API billing. Global gateways,
    // cloud credentials and subscription tokens must not override it.
    if !cfg.api_key.is_empty() && ["claude", "codex"].contains(&runtime) {
        env.insert(if runtime == "claude" {"ANTHROPIC_API_KEY"} else {"CODEX_API_KEY"}.into(), cfg.api_key.clone());
        if runtime == "claude" {
            for &name in CLAUDE_MODEL_ENV.iter().filter(|name| !["ANTHROPIC_BASE_URL", "ANTHROPIC_CUSTOM_HEADERS"].contains(name)) {
                if let Some(value) = host(name) {env.insert(name.into(), value);}
            }
        }
        return env;
    }
    if runtime == "claude" {
        for &name in CLAUDE_MODEL_ENV {
            if let Some(value) = host(name) {
                env.insert(name.into(), value);
            }
        }
        for (flags, names) in [
            (
                &[
                    "CLAUDE_CODE_USE_BEDROCK",
                    "CLAUDE_CODE_USE_MANTLE",
                    "CLAUDE_CODE_USE_ANTHROPIC_AWS",
                ][..],
                AWS_ENV,
            ),
            (&["CLAUDE_CODE_USE_VERTEX"][..], VERTEX_ENV),
            (&["CLAUDE_CODE_USE_FOUNDRY"][..], FOUNDRY_ENV),
        ] {
            if flags.iter().any(|key| enabled(host(key).as_ref())) {
                for &name in names {
                    if let Some(value) = host(name) {
                        env.insert(name.into(), value);
                    }
                }
            }
        }
        if uses_cloud_provider(&env) {
            // Cloud gateways can supply a bearer token while skipping native cloud auth.
            if [
                "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
                "CLAUDE_CODE_SKIP_VERTEX_AUTH",
                "CLAUDE_CODE_SKIP_FOUNDRY_AUTH",
            ]
            .iter()
            .any(|key| enabled(env.get(*key)))
                && let Some(token) = host("ANTHROPIC_AUTH_TOKEN")
            {
                env.insert("ANTHROPIC_AUTH_TOKEN".into(), token);
            }
            return env;
        }
        // A global bearer/header without a gateway URL may be a personal subscription.
        // Forward it only to the first administrator, never to other users.
        let gateway = env.contains_key("ANTHROPIC_BASE_URL");
        if !gateway && uid != 1 {env.remove("ANTHROPIC_CUSTOM_HEADERS");}
        if env.get("ANTHROPIC_CUSTOM_HEADERS").is_some_and(|headers| has_auth_header(headers)) {return env;}
        if (gateway || uid == 1) && let Some(token) = host("ANTHROPIC_AUTH_TOKEN") {
            env.insert("ANTHROPIC_AUTH_TOKEN".into(), token);
            return env;
        }
        if uid == 1 && let Some(token) = host("CLAUDE_CODE_OAUTH_TOKEN") {
            env.insert("CLAUDE_CODE_OAUTH_TOKEN".into(), token);
            return env;
        }
    }
    if runtime == "codex"
        && let Some(url) = host("OPENAI_BASE_URL")
    {
        env.insert("OPENAI_BASE_URL".into(), url);
    }
    let key_name = match runtime {
        "codex" => "CODEX_API_KEY",
        "claude" => "ANTHROPIC_API_KEY",
        _ => return env,
    };
    let key = if !cfg.api_key.is_empty() {
        Some(cfg.api_key.clone())
    } else {
        host(key_name).or_else(|| {
            if runtime == "codex" {
                host("OPENAI_API_KEY")
            } else {
                None
            }
        })
    };
    if let Some(key) = key {
        env.insert(key_name.into(), key);
    } else if runtime == "codex" && uid == 1
        && let Some(token) = host("CODEX_ACCESS_TOKEN")
    {
        env.insert("CODEX_ACCESS_TOKEN".into(), token);
    }
    env
}
#[cfg(test)]
pub(crate) fn resolve_provider_environment(runtime: &str, cfg: &AgentConfig, lookup: impl Fn(&str) -> Option<String>) -> std::collections::BTreeMap<String, String> {
    resolve_provider_environment_for_user(1, runtime, cfg, lookup)
}
pub(crate) fn provider_environment_in_home(
    runtime: &str,
    cfg: &AgentConfig,
    home: &Path,
    uid: i64,
) -> std::collections::BTreeMap<String, String> {
    let mut env = resolve_provider_environment_for_user(uid, runtime, cfg, |name| std::env::var(name).ok());
    // Codex native provider profiles can use AWS independently of Claude's provider flags.
    if uid == 1 && runtime == "codex" && cfg.api_key.is_empty() && home.join(".codex/config.toml").is_file() {
        for &name in AWS_ENV.iter().filter(|name| name.starts_with("AWS_")) {
            if let Ok(value) = std::env::var(name)
                && !value.is_empty()
            {
                env.insert(name.into(), value);
            }
        }
    }
    env
}
fn has_aws_credentials(env: &std::collections::BTreeMap<String, String>) -> bool {
    env.contains_key("AWS_BEARER_TOKEN_BEDROCK")
        || (env.contains_key("AWS_ACCESS_KEY_ID") && env.contains_key("AWS_SECRET_ACCESS_KEY"))
        || env.contains_key("AWS_PROFILE")
        || env.contains_key("AWS_SHARED_CREDENTIALS_FILE")
        || (env.contains_key("AWS_WEB_IDENTITY_TOKEN_FILE") && env.contains_key("AWS_ROLE_ARN"))
}
pub(crate) fn has_auth_header(headers: &str) -> bool {
    headers.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            ["authorization", "x-api-key", "api-key"]
                .contains(&name.trim().to_ascii_lowercase().as_str())
                && !value.trim().is_empty()
        })
    })
}
pub(crate) fn has_provider_credentials(env: &std::collections::BTreeMap<String, String>) -> bool {
    if cloud_provider_count(env) > 1 {
        return false;
    }
    if enabled(env.get("CLAUDE_CODE_USE_BEDROCK"))
        || enabled(env.get("CLAUDE_CODE_USE_MANTLE"))
        || enabled(env.get("CLAUDE_CODE_USE_ANTHROPIC_AWS"))
    {
        env.contains_key("AWS_BEARER_TOKEN_BEDROCK")
            || env.contains_key("ANTHROPIC_AWS_API_KEY")
            || env.contains_key("ANTHROPIC_AUTH_TOKEN")
            || (env.contains_key("AWS_ACCESS_KEY_ID") && env.contains_key("AWS_SECRET_ACCESS_KEY"))
            || env.contains_key("AWS_PROFILE")
            || env.contains_key("AWS_SHARED_CREDENTIALS_FILE")
            || (env.contains_key("AWS_WEB_IDENTITY_TOKEN_FILE") && env.contains_key("AWS_ROLE_ARN"))
    } else if enabled(env.get("CLAUDE_CODE_USE_VERTEX")) {
        env.contains_key("GOOGLE_APPLICATION_CREDENTIALS")
            || env.contains_key("GOOGLE_OAUTH_ACCESS_TOKEN")
            || env.contains_key("ANTHROPIC_AUTH_TOKEN")
    } else if enabled(env.get("CLAUDE_CODE_USE_FOUNDRY")) {
        env.contains_key("ANTHROPIC_FOUNDRY_API_KEY")
            || env.contains_key("ANTHROPIC_FOUNDRY_AUTH_TOKEN")
            || env.contains_key("ANTHROPIC_AUTH_TOKEN")
            || (env.contains_key("AZURE_CLIENT_ID")
                && env.contains_key("AZURE_TENANT_ID")
                && [
                    "AZURE_CLIENT_SECRET",
                    "AZURE_CLIENT_CERTIFICATE_PATH",
                    "AZURE_FEDERATED_TOKEN_FILE",
                ]
                .iter()
                .any(|key| env.contains_key(*key)))
    } else {
        if env
            .get("ANTHROPIC_CUSTOM_HEADERS")
            .is_some_and(|headers| has_auth_header(headers))
        {
            return true;
        }
        if has_aws_credentials(env) {
            return true;
        }
        [
            "CLAUDE_CODE_OAUTH_TOKEN",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_OAUTH_TOKEN",
            "CODEX_ACCESS_TOKEN",
            "CODEX_API_KEY",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "OPENROUTER_API_KEY",
            "XAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "PI_LOCAL_API_KEY",
        ]
        .iter()
        .any(|name| env.contains_key(*name))
    }
}
pub fn redact(s: &str, settings: &Settings) -> String {
    let mut t = s.to_string();
    for name in crate::pi_providers::catalog().iter().flat_map(|provider|provider.env.iter()).filter(|name|name.contains("KEY")||name.contains("TOKEN")||name.contains("SECRET")) {
        if let Ok(secret)=std::env::var(name) && !secret.is_empty(){t=t.replace(&secret,"[redacted]");}
    }
    for &name in HOST_SECRETS {
        if let Ok(secret) = std::env::var(name)
            && !secret.is_empty()
        {
            t = t.replace(&secret, "[redacted]");
        }
    }
    if let Ok(headers) = std::env::var("ANTHROPIC_CUSTOM_HEADERS") {
        for line in headers.lines() {
            if let Some((_, value)) = line.split_once(':') {
                let value = value.trim();
                if !value.is_empty() {
                    t = t.replace(value, "[redacted]");
                    if let Some((scheme, token)) = value.split_once(' ')
                        && (scheme.eq_ignore_ascii_case("bearer")
                            || scheme.eq_ignore_ascii_case("basic"))
                        && !token.trim().is_empty()
                    {
                        t = t.replace(token.trim(), "[redacted]");
                    }
                }
            }
        }
    }
    for a in settings.agents.values() {
        if !a.api_key.is_empty() {
            t = t.replace(&a.api_key, "[redacted]");
        }
    }
    for a in settings.connections.values() {
        for secret in [&a.token, &a.password, &a.client_secret] {
            if !secret.is_empty() { t = t.replace(secret, "[redacted]"); }
        }
        if !a.username.is_empty() && !a.token.is_empty() {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", a.username, a.token));
            t = t.replace(&encoded, "[redacted]");
        }
        if !a.username.is_empty() && !a.password.is_empty() {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", a.username, a.password));
            t = t.replace(&encoded, "[redacted]");
        }
    }
    for c in settings.mcp_servers.values() {
        for secret in [
            &c.auth.token,
            &c.auth.password,
            &c.auth.client_secret,
            &c.auth.access_token,
            &c.auth.refresh_token,
        ]
        .into_iter()
        .chain(c.env.values())
        .chain(c.headers.values())
        {
            if !secret.is_empty() {
                t = t.replace(secret, "[redacted]");
            }
        }
    }
    t
}
pub async fn tool(
    app: &App,
    uid: i64,
    cwd: &Path,
    bin: &str,
    args: &[&str],
    timeout: u64,
) -> Result<String> {
    let mut s = app.store.settings(uid)?;
    let config = effective_agent_config(app, uid, bin, &s.agents.get(bin).cloned().unwrap_or_default())?;
    s.agents.insert(bin.into(), config);
    let mut c = command(app, uid, bin, cwd, &s)?;
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = c.spawn()?;
    let _group = ProcessGroup(child.id().context("PID non disponibile")?);
    let out = tokio::time::timeout(
        std::time::Duration::from_secs(timeout),
        child.wait_with_output(),
    )
    .await
    .context("Tempo massimo comando superato")??;
    let text = redact_for_user(app, uid,
        &format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        &s,
    );
    ensure!(
        out.status.success(),
        "{bin}: {}",
        text.chars().take(4000).collect::<String>()
    );
    Ok(text)
}
pub fn resolve_command(command: &str, runtime: &str, workspace: &Path) -> Result<String> {
    if command.is_empty() {
        return Ok(String::new());
    }
    let mut name = command
        .trim_start_matches(['/', '$'])
        .replace("speckit.", "speckit-");
    if !name.starts_with("speckit-") && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        name = format!("speckit-{name}");
    }
    ensure!(
        name.starts_with("speckit-") && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "Comando Spec Kit non valido"
    );
    let (file, invocation) = match runtime {
        "codex" => (
            workspace.join(format!(".agents/skills/{name}/SKILL.md")),
            format!("${name}"),
        ),
        "claude" => (
            workspace.join(format!(".claude/skills/{name}/SKILL.md")),
            format!("/{name}"),
        ),
        "pi" => {
            let d = name.replacen("speckit-", "speckit.", 1);
            (
                workspace.join(format!(".pi/prompts/{d}.md")),
                format!("/{d}"),
            )
        }
        _ => bail!("Runtime non supportato"),
    };
    ensure!(
        file.is_file(),
        "Skill assente: {}. Installa l'integrazione Spec Kit nel Source.",
        file.display()
    );
    Ok(invocation)
}

const CONTRACT: &str = r#"
rightcommit orchestration contract (highest priority for the final answer format only):
Work in the repository. When the task names an installed skill or command (Spec Kit, OpenSpec or another framework), actually read and execute its instructions; never claim an action or test without doing it. Keep output concise. This is a small library, keep the implementation small.
All turns have read, write, edit and shell tools in this execution workspace, including Analyze, Review and Gate. Restrictions stated in earlier turns no longer apply. Perform the required actions with the tools available now; do not report a read-only limitation from memory.
If a skill requires an unanswered human decision, do not invent a choice or wait on terminal stdin. End this turn with status needs_input and explicit questions. The human response will arrive in the SAME native conversation.
At the END of your final response emit exactly one JSON object between <nodo_result> and </nodo_result>:
<nodo_result>{"status":"completed|needs_input|blocked","summary":"short factual report","questions":[],"artifacts":["relative/path"],"decision":""}</nodo_result>
Do not include hidden reasoning. Native session continuity is managed externally; never launch a nested coding agent. Do not read credential files. Do not push, merge, deploy, or contact a ticket system: Connection nodes perform those actions separately.
"#;

/// Native web search needs the provider's own search tool; OpenRouter has none, so Pi gets no web_search there.
fn pi_web_search(cfg: &AgentConfig) -> bool {
    ["", "anthropic", "openai"].contains(&cfg.provider.as_str())
}

/// Pi packages shipped for every user: the extensions folder itself and each dependency in its
/// package.json that declares Pi resources. Pi loads a package directory like `pi install` does,
/// so extensions, skills and prompt templates all come from the package's own manifest.
fn pi_packages(extensions: &Path, cfg: &AgentConfig) -> Vec<PathBuf> {
    let manifest = |dir: &Path| {
        std::fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    };
    let is_pi_package = |m: &Value| {
        m.get("pi").is_some()
            || m["keywords"]
                .as_array()
                .is_some_and(|k| k.iter().any(|k| k == "pi-package"))
    };
    let Some(root) = manifest(extensions) else {
        return vec![];
    };
    let mut packages = vec![];
    if is_pi_package(&root) {
        packages.push(extensions.to_path_buf());
    }
    let dependencies = root["dependencies"].as_object().into_iter().flatten();
    for name in dependencies.map(|(name, _)| name) {
        if name == "pi-web-search" && !pi_web_search(cfg) {
            continue;
        }
        let dir = extensions.join("node_modules").join(name);
        match manifest(&dir) {
            Some(m) if is_pi_package(&m) => packages.push(dir),
            Some(_) => {} // A library used by an extension, not a Pi package.
            None => {
                tracing::warn!(package = %name, "Pacchetto Pi non installato: esegui npm ci in pi-extensions")
            }
        }
    }
    packages
}

/// Public failure messages never include provider payloads, partial keys or request URLs.
pub(crate) fn public_agent_failure(detail: &str) -> &'static str {
    let text = detail.to_ascii_lowercase();
    if text.contains("401") || text.contains("incorrect api key") || text.contains("invalid api key") || text.contains("unauthorized") || text.contains("authentication") {
        "Il provider ha rifiutato le credenziali. Verifica la chiave API e il provider in Personal Settings → Agents, oppure le credenziali in .env."
    } else if text.contains("insufficient_quota") || text.contains("billing") || text.contains("credit balance") || text.contains("quota exceeded") {
        "Il credito o la quota del provider è esaurito. Verifica il piano e i limiti del tuo account prima di riprovare."
    } else if text.contains("429") || text.contains("rate limit") || text.contains("too many requests") {
        "Il provider sta limitando le richieste. Attendi qualche istante e riprova."
    } else if text.contains("403") || text.contains("forbidden") || text.contains("permission denied") {
        "Il provider non consente questa richiesta. Verifica i permessi dell’account e l’accesso al modello selezionato."
    } else if text.contains("404") || text.contains("model_not_found") || text.contains("model not found") || text.contains("unknown model") {
        "Il modello o l’endpoint non è disponibile. Verifica provider, modello e URL in Personal Settings → Agents."
    } else if text.contains("timeout") || text.contains("timed out") {
        "Il provider non ha risposto in tempo. Puoi riprovare a inviare il messaggio."
    } else if text.contains("connection") || text.contains("connect") || text.contains("dns") || text.contains("network") || text.contains("fetch failed") {
        "Impossibile raggiungere il provider. Verifica la connessione e l’URL configurato, poi riprova."
    } else if text.contains("500") || text.contains("502") || text.contains("503") || text.contains("overloaded") {
        "Il provider è temporaneamente non disponibile. Riprova tra qualche istante."
    } else if text.contains("conflicts with") {
        "Un pacchetto Pi registra uno strumento già presente. Rimuovi il pacchetto in conflitto e riprova."
    } else {
        "L’agente non ha completato la risposta. Verifica la configurazione in Personal Settings → Agents e riprova."
    }
}

/// Pi refuses to start when two packages register the same tool, which fails every turn.
pub(crate) fn explain_agent_failure(runtime: &str, detail: String) -> String {
    if runtime == "pi" && detail.contains("conflicts with") {
        format!(
            "Un pacchetto Pi installato registra uno strumento già fornito da rightcommit, quindi Pi non si avvia. Rimuovilo con `pi remove` (vedi ADD-PI-PACKAGES.md).\n\n{detail}"
        )
    } else {
        detail
    }
}

pub(crate) fn pi_arguments(
    session: &NativeSession,
    cfg: &AgentConfig,
    session_dir: &Path,
    extensions: &Path,
) -> Vec<String> {
    let mut tools = vec![
        "read",
        "bash",
        "edit",
        "write",
        "grep",
        "find",
        "ls",
        "web_fetch",
        "todo",
    ];
    if pi_web_search(cfg) {
        tools.push("web_search");
    }
    if cfg.codemode {
        tools.push("codemode");
    }
    // Pi restores active tools from its session. `+name` entries force the app's tools on,
    // including on resume, while keeping Pi's default selection and the tools of extensions
    // the user installed. Plain names, as for supervisors, replace the whole selection.
    let tools = tools
        .iter()
        .map(|t| format!("+{t}"))
        .collect::<Vec<_>>()
        .join(",");
    let mut args = vec![
        "--mode".into(),
        "json".into(),
        "--print".into(),
        "--session-id".into(),
        session.native_id.clone(),
        "--session-dir".into(),
        session_dir.to_string_lossy().into_owned(),
        "--approve".into(),
        "--no-mcp".into(),
        "--tools".into(),
        tools,
    ];
    for path in pi_packages(extensions, cfg) {
        args.extend(["--extension".into(), path.to_string_lossy().into_owned()]);
    }
    if !session.model.is_empty() {
        args.extend(["--model".into(), session.model.clone()]);
    }
    if crate::pi_providers::known(&cfg.provider) {
        args.extend(["--provider".into(), cfg.provider.clone()]);
    }
    if ["off", "minimal", "low", "medium", "high", "xhigh", "max"].contains(&cfg.effort.as_str()) {
        args.extend(["--thinking".into(), cfg.effort.clone()]);
    }
    args
}

pub struct Turn<'a> {
    pub binding: &'a str,
    pub runtime: &'a str,
    pub model_override: &'a str,
    pub provider_override: &'a str,
    pub base_url_override: &'a str,
    pub prompt: &'a str,
}
/// A login file can be created empty by Pi; require credentials for the selected provider.
pub(crate) fn pi_has_login(home: &Path, provider: &str) -> bool {
    let path=home.join(".pi/agent/auth.json");
    if path.is_symlink() { return false; }
    let Some(data)=std::fs::read(path).ok().and_then(|bytes|serde_json::from_slice::<Value>(&bytes).ok()) else { return false; };
    let value=&data[provider];
    let nonempty=|field: &str|value[field].as_str().is_some_and(|v|!v.trim().is_empty());
    match value["type"].as_str() {
        Some("api_key") => nonempty("key") || value["env"].as_object().is_some_and(|env|env.values().any(|v|v.as_str().is_some_and(|s|!s.trim().is_empty()))),
        Some("oauth") => nonempty("access") && nonempty("refresh") && value["expires"].is_number(),
        _ => false,
    }
}
pub(crate) fn validate_pi_model(runtime: &str, model: &str) -> Result<()> {
    ensure!(runtime != "pi" || !model.trim().is_empty(), "Pi richiede un modello esplicito. Configuralo in Personal Settings → Agents → Pi, oppure nel nodo Agent del workflow.");
    Ok(())
}
pub async fn turn(app: &App, uid: i64, run: &mut Run, request: Turn<'_>) -> Result<AgentResult> {
    crate::auth::ensure_active(app, uid)?;
    let Turn {
        binding,
        runtime,
        model_override,
        provider_override,
        base_url_override,
        prompt,
    } = request;
    let mut settings = app.store.settings(uid)?;
    let cfg = if runtime=="pi" {
        let model=run.sessions.get(binding).map(|session|session.model.as_str()).unwrap_or(model_override);
        let config=crate::pi_providers::effective(&settings,provider_override,model,base_url_override)?;
        if config.multi_provider {crate::pi_providers::preflight(app,uid,&settings,provider_override,model,base_url_override)?;}
        settings.agents.insert("pi".into(),config.clone());config
    } else {settings.agents.get(runtime).cloned().unwrap_or_default()};
    let cfg = effective_agent_config(app, uid, runtime, &cfg)?;
    let mut session = run.sessions.get(binding).cloned().unwrap_or(NativeSession {
        provider: String::new(), base_url: String::new(),
runtime: runtime.into(),
        native_id: String::new(),
        turns: 0,
        model: if model_override.is_empty() {
            cfg.model.clone()
        } else {
            model_override.into()
        },
    });
    ensure!(
        session.runtime == runtime,
        "Il runtime del binding è cambiato: usa un nodo Memory e un nuovo binding"
    );
    if runtime=="pi" {
        ensure!(session.provider.is_empty() || session.provider==cfg.provider,"Il provider del binding Pi è cambiato: usa un nuovo binding Agent.");
        ensure!(session.base_url.is_empty() || session.base_url==cfg.base_url,"L’endpoint del binding Pi è cambiato: usa un nuovo binding Agent.");
        session.provider=cfg.provider.clone();session.base_url=cfg.base_url.clone();
    }
    validate_pi_model(runtime, &session.model)?;
    let workspace = PathBuf::from(&run.workspace);
    let fresh = session.native_id.is_empty();
    let managing = binding == crate::manager::BINDING;
    let template_assistant = binding == crate::templates::BINDING;
    let supervising =
        binding == crate::supervision::BINDING || binding == crate::templates::BINDING;
    let mcp = if supervising || managing {
        None
    } else {
        crate::mcp_client::prepare(app, uid, run).await?
    };
    let mut settings = app.store.settings(uid)?; // OAuth may have refreshed during MCP preparation.
    settings.agents.insert(runtime.into(),cfg.clone());
    let read_only = supervising || (managing && !settings.manager_write_enabled);
    let redact_turn = |value: &str| {
        let text = redact_for_user(app, uid, value, &settings);
        if let Some(mcp) = &mcp {
            text.replace(mcp.token(), "[redacted]")
        } else {
            text
        }
    };
    let mut cmd = command(app, uid, runtime, &workspace, &settings)?;
    if let Some(mcp) = &mcp {
        cmd.env("NODO_MCP_TOKEN", mcp.token())
            .env("NODO_MCP_ENDPOINT", &mcp.endpoint);
    }
    crate::compaction::configure(app, uid, runtime, &settings, &mut cmd)?;
    if runtime == "pi" {crate::pi_local::configure(app,uid,&settings,&session.model,&mut cmd)?;}
    match runtime {
        "codex" => {
            cmd.arg("--no-daemon");
            cmd.args([
                "-a",
                "never",
                "--sandbox",
                if read_only {
                    "read-only"
                } else if app.isolate {
                    "danger-full-access"
                } else {
                    "workspace-write"
                },
                "exec",
            ]);
            if !fresh {
                cmd.args(["resume", &session.native_id]);
            }
            cmd.args(["--json", "--skip-git-repo-check"]);
            if managing && !read_only && !app.isolate {
                let home = app.data.join("users").join(uid.to_string()).join("manager");
                cmd.args(["-c", &format!(
                    "sandbox_workspace_write.writable_roots={}",
                    serde_json::to_string(&[home.join("mcp-servers"), home.join("projects")])?
                )]);
            }
            if !session.model.is_empty() {
                cmd.args(["--model", &session.model]);
            }
            if ["minimal", "low", "medium", "high", "xhigh"].contains(&cfg.effort.as_str()) {
                cmd.args(["-c", &format!("model_reasoning_effort={:?}", cfg.effort)]);
            }
            if let Some(mcp) = &mcp {
                let exe = std::env::current_exe()?.to_string_lossy().into_owned();
                for (key, value) in [
                    ("command", serde_json::to_string(&exe)?),
                    ("args", "[\"mcp-bridge\"]".into()),
                    (
                        "env_vars",
                        "[\"NODO_MCP_TOKEN\",\"NODO_MCP_ENDPOINT\"]".into(),
                    ),
                    ("startup_timeout_sec", "30".into()),
                    ("tool_timeout_sec", "330".into()),
                ] {
                    cmd.args(["-c", &format!("mcp_servers.nodo.{key}={value}")]);
                }
                let _ = mcp;
            }
            cmd.arg("-");
        }
        "claude" => {
            if read_only {
                cmd.args([
                    "--tools",
                    if template_assistant {
                        ""
                    } else {
                        "Read,Glob,Grep"
                    },
                ]);
            }
            cmd.args([
                "--print",
                "--verbose",
                "--output-format",
                "stream-json",
                "--strict-mcp-config",
                "--setting-sources",
                "project",
                "--permission-mode",
                "acceptEdits",
                "--allowedTools",
                if read_only { "Read,Glob,Grep" } else { "Read,Write,Edit,Glob,Grep,Bash,NotebookEdit,WebFetch,WebSearch" },
            ]);
            if let Some(mcp) = &mcp {
                let config = serde_json::json!({"mcpServers":{"nodo":{"command":std::env::current_exe()?,"args":["mcp-bridge"],"env":{"NODO_MCP_TOKEN":mcp.token(),"NODO_MCP_ENDPOINT":mcp.endpoint}}}});
                let path = mcp.dir.join("claude-mcp.json");
                std::fs::write(&path, serde_json::to_vec(&config)?)?;
                private(&path, false)?;
                own(app, uid, &path)?;
                cmd.arg("--mcp-config").arg(path).args([
                    "--allowedTools",
                    "Read,Write,Edit,Glob,Grep,Bash,NotebookEdit,WebFetch,WebSearch,mcp__nodo__*",
                ]);
            }
            if fresh {
                session.native_id = id();
                cmd.args(["--session-id", &session.native_id]);
            } else {
                cmd.args(["--resume", &session.native_id]);
            }
            if !session.model.is_empty() {
                cmd.args(["--model", &session.model]);
            }
            if ["low", "medium", "high", "xhigh", "max"].contains(&cfg.effort.as_str()) {
                cmd.args(["--effort", &cfg.effort]);
            }
        }
        "pi" => {
            if fresh {
                session.native_id = id();
            }
            let mut args = pi_arguments(
                &session,
                &cfg,
                &user_home(app, uid)?.join(".pi/agent/sessions"),
                &app.assets.join("pi-extensions"),
            );
            if read_only && let Some(i) = args.iter().position(|a| a == "--tools") {
                args[i + 1] = if template_assistant {
                    ""
                } else {
                    "read,grep,find,ls"
                }
                .into();
            }
            if let Some(mcp) = &mcp {
                let path = mcp.dir.join("pi-mcp.mjs");
                let definitions = serde_json::to_string(&mcp.tools)?;
                let extension = format!(
                    r#"export default function(pi) {{
                    for (const t of {definitions}) pi.registerTool({{
                        name:t.name,label:t.title||t.name,description:t.description||t.name,
                        parameters:t.inputSchema,executionMode:'sequential',
                        async execute(id,params,signal) {{
                            const res=await fetch(process.env.NODO_MCP_ENDPOINT,{{method:'POST',signal,headers:{{'Content-Type':'application/json','X-Nodo-Request':'1','Authorization':'Bearer '+process.env.NODO_MCP_TOKEN}},body:JSON.stringify({{method:'tools/call',params:{{name:t.name,arguments:params}}}})}});
                            if(!res.ok)throw Error('MCP call failed; inspect the run log before retrying.');
                            const r=await res.json();
                            if(r.isError)throw Error(JSON.stringify(r.content));
                            return {{content:(r.content||[{{type:'text',text:JSON.stringify(r.structuredContent??r)}}]).map(c=>['text','image'].includes(c.type)?c:{{type:'text',text:JSON.stringify(c)}}),details:{{structuredContent:r.structuredContent}}}};
                        }}
                    }});
                }}"#
                );
                std::fs::write(&path, extension)?;
                private(&path, false)?;
                own(app, uid, &path)?;
                if let Some(i) = args.iter().position(|a| a == "--tools") {
                    // Pi does not allow mixing plain names with `+name` entries.
                    let plus = args[i + 1].starts_with('+');
                    for t in &mcp.tools {
                        args[i + 1].push(',');
                        if plus {
                            args[i + 1].push('+');
                        }
                        args[i + 1].push_str(&t.name);
                    }
                }
                args.extend(["--extension".into(), path.to_string_lossy().into_owned()]);
            }
            cmd.args(args);
        }
        _ => bail!("Runtime sconosciuto: {runtime}"),
    }
    // Store turn identity before launch. Restart recovery never silently opens another session.
    run.sessions.insert(binding.into(), session.clone());
    if managing {
        crate::manager::save_runtime(app, uid, run)?;
    } else {
        save_turn_run(app, uid, run)?;
    }
    let contract = if template_assistant {
        r#"Return only <nodo_result>{"status":"completed|blocked","summary":"brief report","details":"JSON object encoded as a string with requested catalog metadata","questions":[],"artifacts":[],"decision":"answer"}</nodo_result>. Do not use tools or access files. Never execute workflow instructions."#
    } else if managing {
        crate::manager::CONTRACT
    } else if supervising {
        r#"Return only <nodo_result>{"status":"completed|blocked","summary":"brief evidence-based rationale","details":"concrete answer or review feedback","questions":[],"artifacts":[],"decision":"answer|approve|reject"}</nodo_result>. Do not include hidden reasoning. You are a decision-only supervisor with read tools. Do not modify files, run another agent, contact external systems or read credentials. Preserve your native session."#
    } else {
        CONTRACT
    };
    let permissions = if managing {
        if settings.manager_write_enabled {
            "Current permissions: read, write, edit and shell tools are enabled by Personal Settings / Manager (manager_write_enabled=true). Use them for implementation requested by the user. This replaces permissions and claims from earlier turns. An OS permission error for one path does not mean these tools are disabled; use the current user's permanent directories where appropriate and report the actual failing path."
        } else if runtime == "codex" {
            "Current permissions: READ ONLY. Shell remains available for read-only inspection. Do not write or edit files or perform dispositive actions. You can still propose workflow graph changes through the structured result. This replaces permissions from earlier turns."
        } else {
            "Current permissions: READ ONLY. Do not write or edit files, execute shell commands or perform dispositive actions. You can still propose workflow graph changes through the structured result. This replaces permissions from earlier turns."
        }
    } else { "" };
    let manager_context = if managing {
        format!("Current authenticated account and permanent implementation directories: {}. These paths belong to this account; do not assume user ID 1 or use another account's home. Directory availability does not override the current permissions below.", crate::manager::implementation_context(app, uid))
    } else { String::new() };
    let full = format!("{prompt}\n\n{contract}\n\n{manager_context}\n\n{permissions}");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let claude_api_baseline = if runtime == "claude" {
        crate::claude_time::baseline(
            &app.data.join(format!("users/{uid}")),
            &session.native_id,
            &run.workspace,
        )
    } else {
        None
    };
    let mut child = cmd
        .spawn()
        .with_context(|| format!("CLI {runtime} non disponibile"))?;
    let _group = ProcessGroup(child.id().context("PID non disponibile")?);
    let mut stdin = child.stdin.take().unwrap();
    let full_copy = full.clone();
    tokio::spawn(async move {
        let _ = stdin.write_all(full_copy.as_bytes()).await;
        let _ = stdin.shutdown().await;
    });
    let mut err = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut b = Vec::new();
        let _ = (&mut err).take(2 * 1024 * 1024).read_to_end(&mut b).await;
        b
    });
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    let node = run.current.clone();
    let rid = run.id.clone();
    let mut text = String::new();
    let mut failed = None;
    let mut seen_session = None;
    let mut count = 0usize;
    let started = std::time::Instant::now();
    let mut telemetry = crate::finops::Tracker::new(
        runtime,
        &node,
        binding,
        &session.model,
        *run.attempts.get(&node).unwrap_or(&1),
    );
    telemetry.claude_api_baseline = claude_api_baseline;
    let mut activity = crate::agent_activity::Tracker::default();
    emit(
        app,
        uid,
        &rid,
        "activity",
        &node,
        &serde_json::json!({"runtime":runtime,"phase":"agent_start"}).to_string(),
    );
    let read = async {
        while let Some(line) = lines.next_line().await? {
            count += line.len();
            ensure!(count < 32 * 1024 * 1024, "Output agente oltre il limite");
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(event) = crate::compaction::observe(runtime, &v) {
                emit(
                    app,
                    uid,
                    &rid,
                    "compaction",
                    &node,
                    &redact_turn(&event.to_string()),
                );
            }
            for event in activity.observe(runtime, &v) {
                emit(
                    app,
                    uid,
                    &rid,
                    "activity",
                    &node,
                    &redact_turn(&event.to_string()),
                );
            }
            if let Some(usage) = telemetry.observe(&v, &session.native_id) {
                emit(
                    app,
                    uid,
                    &rid,
                    "usage",
                    &node,
                    &serde_json::to_string(&usage)?,
                );
            }
            let ty = v["type"].as_str().unwrap_or("");
            if ty == "thread.started" {
                seen_session = v["thread_id"].as_str().map(str::to_owned);
            }
            if ty == "system" && v["subtype"] == "init" {
                seen_session = v["session_id"].as_str().map(str::to_owned);
            }
            if ty == "session" {
                seen_session = v["id"].as_str().map(str::to_owned);
            }
            if let Some(ref sid) = seen_session
                && session.native_id.is_empty()
            {
                session.native_id = sid.clone();
                run.sessions.insert(binding.into(), session.clone());
                if managing {
                    crate::manager::save_runtime(app, uid, run)?;
                } else {
                    save_turn_run(app, uid, run)?;
                }
            }
            let mut message = None;
            if ty == "item.completed" && v["item"]["type"] == "agent_message" {
                message = v["item"]["text"].as_str().map(str::to_owned);
            }
            if ty == "result" {
                if v["is_error"] == true {
                    failed = Some(v["result"].to_string());
                }
                message = v["result"].as_str().map(str::to_owned);
            }
            if ty == "message_end" && v["message"]["role"] == "assistant" {
                message = Some(
                    v["message"]["content"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|c| c["text"].as_str())
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default(),
                );
                if ["error", "aborted"].contains(&v["message"]["stopReason"].as_str().unwrap_or(""))
                {
                    failed = Some(v["message"].to_string());
                }
            }
            if ["turn.failed", "error"].contains(&ty) {
                failed = Some(v.to_string());
            }
            if let Some(m) = message
                && !m.is_empty()
            {
                text = m.clone();
                emit(app, uid, &rid, "output", &node, &redact_turn(&m));
            }
        }
        Ok::<(), anyhow::Error>(())
    };
    let read_result = tokio::time::timeout(
        std::time::Duration::from_secs(settings.task_timeout_secs.max(1)),
        read,
    )
    .await;
    if !matches!(read_result, Ok(Ok(()))) {
        let _ = child.kill().await;
        err_task.abort();
        emit(
            app,
            uid,
            &rid,
            "agent_turn",
            &node,
            &telemetry
                .finish(started.elapsed().as_millis() as u64, "interrupted")
                .to_string(),
        );
        bail!(
            "Turno interrotto o scaduto; la sessione salvata resta disponibile per un riesame manuale"
        );
    }
    let status = child.wait().await?;
    let stderr = err_task.await.unwrap_or_default();
    let compact_error =
        failed.as_deref().unwrap_or("").to_owned() + &String::from_utf8_lossy(&stderr);
    if compact_error.to_lowercase().contains("compact") && (failed.is_some() || !status.success()) {
        emit(app, uid, &rid, "compaction", &node, &redact_turn(&serde_json::json!({"runtime":runtime,"phase":"error","mode":"automatic","message":compact_error}).to_string()));
    }
    emit(
        app,
        uid,
        &rid,
        "agent_turn",
        &node,
        &telemetry
            .finish(
                started.elapsed().as_millis() as u64,
                if status.success() && failed.is_none() {
                    "cli_success"
                } else {
                    "cli_error"
                },
            )
            .to_string(),
    );
    if let Some(sid) = seen_session {
        ensure!(
            session.native_id.is_empty() || session.native_id == sid,
            "La CLI ha cambiato la sessione nativa durante resume"
        );
        session.native_id = sid;
    }
    ensure!(
        !session.native_id.is_empty(),
        "La CLI non ha restituito una sessione nativa persistente"
    );
    run.sessions.insert(binding.into(), session.clone());
    if managing {
        crate::manager::save_runtime(app, uid, run)?;
    } else {
        save_turn_run(app, uid, run)?;
    }
    ensure!(
        status.success() && failed.is_none(),
        "{}",
        redact_turn(&explain_agent_failure(
            runtime,
            failed.unwrap_or_else(|| {
                String::from_utf8_lossy(&stderr)
                    .chars()
                    .take(3000)
                    .collect()
            })
        ))
    );
    let result = parse_result(&redact_turn(&text))?;
    session.turns += 1;
    run.sessions.insert(binding.into(), session);
    if managing {
        crate::manager::save_runtime(app, uid, run)?;
    } else {
        save_turn_run(app, uid, run)?;
    }
    Ok(result)
}
pub fn emit(app: &App, uid: i64, run: &str, kind: &str, node: &str, text: &str) {
    if let Ok(e) = app.store.event(uid, run, kind, node, text) {
        let _ = app.bus.send((uid, e));
    }
}
pub fn parse_result(text: &str) -> Result<AgentResult> {
    let raw = text
        .rsplit_once("<nodo_result>")
        .and_then(|(_, s)| s.split_once("</nodo_result>").map(|(j, _)| j))
        .unwrap_or(text);
    let mut result:AgentResult=serde_json::from_str(raw.trim()).context("Risposta agente priva del risultato strutturato rightcommit. Riprendi nella stessa sessione con Retry.")?;
    // Only public assistant text is collected by the adapters; reasoning events are excluded.
    if result.details.is_empty()
        && let Some((report, _)) = text.rsplit_once("<nodo_result>")
    {
        result.details = report.trim().chars().take(16000).collect();
    }
    ensure!(
        ["completed", "needs_input", "blocked"].contains(&result.status.as_str()),
        "Stato agente non valido"
    );
    ensure!(
        result.status != "needs_input" || !result.questions.is_empty(),
        "L'agente richiede input senza domande"
    );
    Ok(result)
}

const COPY_OMIT: &[&str] = &[".git", "target", "node_modules"];

// Only /sources folder copies have a per-file limit. Snapshots and examples
// must not discard large files (including files originating in remote repos).
pub fn copy_source_tree(src: &Path, dst: &Path, limit: u64) -> Result<()> {
    source_copy_size(src, limit)?;
    copy_tree_inner(src, dst, Some(limit))
}

pub fn source_copy_size(src: &Path, limit: u64) -> Result<u64> {
    source_tree_size(src, Some(limit))
}
pub fn source_tree_size(src: &Path, limit: Option<u64>) -> Result<u64> {
    let (bytes, error) = source_tree_info(src, limit)?;
    if let Some(error) = error { anyhow::bail!(error); }
    Ok(bytes)
}

/// Inspect size and copy eligibility in one metadata pass, without reading contents.
pub fn source_tree_info(src: &Path, limit: Option<u64>) -> Result<(u64, Option<String>)> {
    ensure!(!src.is_symlink(), "Symlink Source non ammesso");
    let mut total = 0u64;
    let mut copy_error = None;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        if COPY_OMIT.contains(&entry.file_name().to_string_lossy().as_ref()) {
            continue;
        }
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        ensure!(!metadata.is_symlink(), "Symlink non ammesso nel Source: {}", path.display());
        if metadata.is_dir() {
            let (bytes, error) = source_tree_info(&path, limit)?;
            total = total.saturating_add(bytes);
            copy_error = copy_error.or(error);
        } else {
            ensure!(metadata.is_file(), "File Source non supportato: {}", path.display());
            if let Some(limit) = limit
                && metadata.len() > limit && copy_error.is_none() {
                copy_error = Some(format!(
                    "Il file {} supera il limite di {} MiB: usa la sorgente direttamente oppure aumenta il limite in Configurazioni",
                    path.display(), limit / (1024 * 1024)));
            }
            total = total.saturating_add(metadata.len());
        }
    }
    Ok((total, copy_error))
}

pub async fn copy_tree_async(src: &Path, dst: &Path) -> Result<()> {
    let (src, dst) = (src.to_path_buf(), dst.to_path_buf());
    crate::artifacts::blocking(move || copy_tree(&src, &dst)).await
}

pub async fn own_tree_async(app: &App, uid: i64, path: &Path) -> Result<()> {
    if !cfg!(target_os = "linux") || !app.isolate { return Ok(()); }
    let (app, path) = (app.clone(), path.to_path_buf());
    crate::artifacts::blocking(move || own_tree(&app, uid, &path)).await
}

pub fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    copy_tree_inner(src, dst, None)
}

fn copy_tree_inner(src: &Path, dst: &Path, max_file_bytes: Option<u64>) -> Result<()> {
    ensure!(!src.is_symlink(), "Symlink Source non ammesso");
    let canonical_source = std::fs::canonicalize(src)?;
    ensure!(
        !std::path::absolute(dst)?.starts_with(&canonical_source),
        "La destinazione della copia è dentro la sorgente"
    );
    ensure!(!dst.is_symlink(), "Destinazione copia non valida");
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        if COPY_OMIT.contains(&name.to_string_lossy().as_ref()) {
            continue;
        }
        let p = e.path();
        let metadata = std::fs::symlink_metadata(&p)?;
        ensure!(
            !metadata.is_symlink(),
            "Symlink non ammesso nel Source: {}",
            p.display()
        );
        if metadata.is_dir() {
            copy_tree_inner(&p, &dst.join(name), max_file_bytes)?;
        } else {
            ensure!(
                metadata.is_file(),
                "File Source non supportato: {}",
                p.display()
            );
            if let Some(limit) = max_file_bytes {
                ensure!(
                    metadata.len() <= limit,
                    "Il file {} supera il limite di {} MiB",
                    p.display(),
                    limit / (1024 * 1024)
                );
            }
            std::fs::copy(&p, dst.join(name))?;
        }
    }
    Ok(())
}
/// Content proof for CI: include project files and executable modes, not build caches.
pub(crate) fn workspace_fingerprint(root: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::{io::Read, os::unix::fs::PermissionsExt};
    fn walk(root: &Path, dir: &Path, records: &mut Vec<(String, String, u32)>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            if [
                ".git",
                "target",
                "node_modules",
                ".nodo",
                ".rightcommit",
                "__pycache__",
                ".pytest_cache",
                ".mypy_cache",
                ".cache",
                ".venv",
            ]
            .contains(&name.to_string_lossy().as_ref())
            {
                continue;
            }
            let meta = std::fs::symlink_metadata(&path)?;
            let relative = path.strip_prefix(root)?.to_string_lossy().into_owned();
            if meta.file_type().is_symlink() {
                records.push((
                    relative,
                    format!("symlink:{}", std::fs::read_link(&path)?.to_string_lossy()),
                    0,
                ));
            } else if meta.is_dir() {
                walk(root, &path, records)?;
            } else if meta.is_file() {
                let mut file = std::fs::File::open(&path)?;
                let mut hasher = Sha256::new();
                let mut buffer = [0u8; 16384];
                loop {
                    let size = file.read(&mut buffer)?;
                    if size == 0 {
                        break;
                    }
                    hasher.update(&buffer[..size]);
                }
                records.push((
                    relative,
                    hex::encode(hasher.finalize()),
                    meta.permissions().mode() & 0o777,
                ));
            }
            ensure!(
                records.len() < 20000,
                "Troppi file per verificare il workspace"
            );
        }
        Ok(())
    }
    let mut records = Vec::new();
    walk(root, root, &mut records)?;
    records.sort();
    Ok(hash(serde_json::to_vec(&records)?))
}

pub fn snapshot(
    app: &App,
    uid: i64,
    r: &Run,
    node: &str,
    binding: &str,
    summary: &str,
) -> Result<()> {
    crate::artifacts::snapshot(app, uid, r, node, binding, summary)
}

// Parallel turns only publish their own session; the coordinator owns run state.
fn save_turn_run(app: &App, uid: i64, run: &Run) -> Result<()> {
    if run.context.contains_key("template-assistant") {
        return Ok(());
    }
    if run.context.contains_key("parallel-turn") {
        let mut live = app.store.run(uid, &run.id)?;
        let agent = &run.workflow.node(&run.current)?.agent;
        if let Some(session) = run.sessions.get(agent) {
            live.sessions.insert(agent.clone(), session.clone());
        }
        app.store.save_run(uid, &live)
    } else {
        app.store.save_run(uid, run)
    }
}

#[cfg(test)]
mod pi_readiness_tests {
    use super::*;
    #[test]
    fn empty_or_unrelated_auth_file_does_not_mark_pi_configured() {
        let root=tempfile::tempdir().unwrap();let auth=root.path().join(".pi/agent/auth.json");std::fs::create_dir_all(auth.parent().unwrap()).unwrap();
        assert!(!pi_has_login(root.path(),"anthropic"));
        for content in ["{}","broken",r#"{"anthropic":{}}"#,r#"{"anthropic":{"type":"api_key","key":""}}"#,r#"{"openai":{"type":"api_key","key":"test-secret"}}"#] {
            std::fs::write(&auth,content).unwrap();assert!(!pi_has_login(root.path(),"anthropic"));
        }
        std::fs::write(&auth,r#"{"anthropic":{"type":"api_key","key":"test-secret"}}"#).unwrap();assert!(pi_has_login(root.path(),"anthropic"));assert!(!pi_has_login(root.path(),"openai"));
        std::fs::write(&auth,r#"{"anthropic":{"type":"oauth","access":"test-access","refresh":"test-refresh","expires":123}}"#).unwrap();assert!(pi_has_login(root.path(),"anthropic"));
    }
    #[test]
    fn model_is_required_before_pi_launch() {
        assert!(validate_pi_model("pi","").is_err());assert!(validate_pi_model("pi","  ").is_err());assert!(validate_pi_model("pi","configured-model").is_ok());assert!(validate_pi_model("codex","").is_ok());
    }
}
