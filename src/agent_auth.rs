use crate::{
    App, agents,
    model::{AgentConfig, Settings},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, process::Stdio};

type Env = BTreeMap<String, String>;
fn on(env: &Env, key: &str) -> bool {
    env.get(key)
        .is_some_and(|v| ["1", "true", "yes", "on"].contains(&v.to_ascii_lowercase().as_str()))
}
fn info(method: &str, source: &str, provider: &str) -> Value {
    json!({"method":method,"source":source,"provider":provider})
}
fn aws_source(env: &Env) -> &str {
    [
        "AWS_BEARER_TOKEN_BEDROCK",
        "ANTHROPIC_AWS_API_KEY",
        "AWS_ACCESS_KEY_ID",
        "AWS_WEB_IDENTITY_TOKEN_FILE",
        "AWS_PROFILE",
        "AWS_SHARED_CREDENTIALS_FILE",
    ]
    .into_iter()
    .find(|key| env.contains_key(*key))
    .unwrap_or("native_credentials")
}
fn claude(cfg: &AgentConfig, env: &Env, native_login: bool) -> Value {
    let aws = [
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_MANTLE",
        "CLAUDE_CODE_USE_ANTHROPIC_AWS",
    ]
    .iter()
    .any(|key| on(env, key));
    let vertex = on(env, "CLAUDE_CODE_USE_VERTEX");
    let foundry = on(env, "CLAUDE_CODE_USE_FOUNDRY");
    if [aws, vertex, foundry]
        .into_iter()
        .filter(|active| *active)
        .count()
        > 1
    {
        return info("configuration_error", "conflicting_providers", "unknown");
    }
    if aws {
        let provider = if on(env, "CLAUDE_CODE_USE_ANTHROPIC_AWS") {
            "anthropic_aws"
        } else if on(env, "CLAUDE_CODE_USE_MANTLE") {
            "bedrock_mantle"
        } else {
            "bedrock"
        };
        let source = if env.contains_key("ANTHROPIC_AUTH_TOKEN") {
            "ANTHROPIC_AUTH_TOKEN"
        } else {
            aws_source(env)
        };
        return info("aws_credentials", source, provider);
    }
    if vertex {
        let source = [
            "ANTHROPIC_AUTH_TOKEN",
            "GOOGLE_OAUTH_ACCESS_TOKEN",
            "GOOGLE_APPLICATION_CREDENTIALS",
        ]
        .into_iter()
        .find(|key| env.contains_key(*key))
        .unwrap_or("native_credentials");
        return info("google_credentials", source, "vertex");
    }
    if foundry {
        let source = [
            "ANTHROPIC_FOUNDRY_AUTH_TOKEN",
            "ANTHROPIC_FOUNDRY_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "AZURE_CLIENT_SECRET",
            "AZURE_FEDERATED_TOKEN_FILE",
        ]
        .into_iter()
        .find(|key| env.contains_key(*key))
        .unwrap_or("native_credentials");
        return info("azure_credentials", source, "foundry");
    }
    let provider = if env.contains_key("ANTHROPIC_BASE_URL") {
        "gateway"
    } else {
        "anthropic"
    };
    if env
        .get("ANTHROPIC_CUSTOM_HEADERS")
        .is_some_and(|headers| agents::has_auth_header(headers))
    {
        return info("auth_headers", "ANTHROPIC_CUSTOM_HEADERS", provider);
    }
    if env.contains_key("ANTHROPIC_AUTH_TOKEN") {
        return info("bearer_token", "ANTHROPIC_AUTH_TOKEN", provider);
    }
    if env.contains_key("CLAUDE_CODE_OAUTH_TOKEN") {
        return info("oauth_token", "CLAUDE_CODE_OAUTH_TOKEN", provider);
    }
    if env.contains_key("ANTHROPIC_API_KEY") {
        return info(
            "api_key",
            if cfg.api_key.is_empty() {
                "ANTHROPIC_API_KEY"
            } else {
                "personal_settings"
            },
            provider,
        );
    }
    if native_login {
        info("native_login", "native_credentials", provider)
    } else {
        info("not_configured", "none", provider)
    }
}
fn codex(
    cfg: &AgentConfig,
    env: &Env,
    native: &Value,
    login: &Value,
    host_codex_key: bool,
) -> Value {
    let provider = native["provider"].as_str().unwrap_or("openai");
    if native["config_error"] == true {
        return info("configuration_error", "native_config", "unknown");
    }
    // A native cloud/custom provider selects its own authentication, independently of OpenAI login.
    if native["aws"] == true {
        return info("aws_credentials", aws_source(env), provider);
    }
    if provider != "openai" {
        return info(
            "native_provider",
            native["env_key"].as_str().unwrap_or("native_config"),
            provider,
        );
    }
    if env.contains_key("CODEX_API_KEY") {
        let source = if !cfg.api_key.is_empty() {
            "personal_settings"
        } else if host_codex_key {
            "CODEX_API_KEY"
        } else {
            "OPENAI_API_KEY"
        };
        return info("api_key", source, provider);
    }
    if env.contains_key("CODEX_ACCESS_TOKEN") {
        return info("access_token", "CODEX_ACCESS_TOKEN", provider);
    }
    match login["kind"].as_str().unwrap_or("none") {
        "chatgpt" => info("chatgpt_login", "login_file", provider),
        "api_key" => info("api_key", "login_file", provider),
        "unknown" => info("native_login", "login_file", provider),
        _ => info("not_configured", "none", provider),
    }
}
fn login_file(home: &Path) -> Value {
    let path = home.join(".codex/auth.json");
    let exists = path.is_file();
    let data = std::fs::metadata(&path)
        .ok()
        .filter(|m| m.len() <= 1024 * 1024)
        .and_then(|_| std::fs::read(&path).ok())
        .and_then(|data| serde_json::from_slice::<Value>(&data).ok());
    let kind = match data {
        Some(v)
            if v["tokens"]["access_token"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
                || v["tokens"]["id_token"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty()) =>
        {
            "chatgpt"
        }
        Some(v) if v["OPENAI_API_KEY"].as_str().is_some_and(|s| !s.is_empty()) => "api_key",
        _ if exists => "unknown",
        _ => "none",
    };
    json!({"path":path,"exists":exists,"kind":kind})
}
async fn native_codex(app: &App, uid: i64, home: &Path, settings: &Settings) -> Value {
    if !home.join(".codex/config.toml").is_file() {
        return json!({"provider":"openai"});
    }
    // Parse native TOML with the Python runtime already included in the image. Only names/flags
    // leave this process; keys, headers, commands and credential contents are never returned.
    let script = r#"import json,pathlib,re,tomllib
try:
 p=pathlib.Path.home()/'.codex/config.toml'
 if p.stat().st_size>262144:raise ValueError()
 c=tomllib.loads(p.read_text())
 profile=c.get('profile')
 if isinstance(profile,str):c.update(c.get('profiles',{}).get(profile,{}))
 provider=c.get('model_provider','openai')
 if not isinstance(provider,str) or not re.fullmatch(r'[A-Za-z0-9._-]{1,100}',provider):raise ValueError()
 definition=c.get('model_providers',{}).get(provider,{})
 key=definition.get('env_key')
 if not isinstance(key,str) or not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]{0,100}',key):key=None
 print(json.dumps({'provider':provider,'aws':provider.startswith('amazon-bedrock') or bool(definition.get('aws')),'env_key':key}))
except Exception:print(json.dumps({'config_error':True}))
"#;
    let Ok(mut cmd) = agents::command(app, uid, "python3", home, settings) else {
        return json!({"config_error":true});
    };
    cmd.args(["-c", script])
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    match tokio::time::timeout(std::time::Duration::from_secs(3), cmd.output()).await {
        Ok(Ok(output)) if output.status.success() => {
            serde_json::from_slice(&output.stdout).unwrap_or(json!({"config_error":true}))
        }
        _ => json!({"config_error":true}),
    }
}
pub async fn describe(app: &App, uid: i64, settings: &Settings) -> anyhow::Result<Value> {
    let home = agents::user_home(app, uid)?;
    let claude_cfg = settings.agents.get("claude").cloned().unwrap_or_default();
    let codex_cfg = settings.agents.get("codex").cloned().unwrap_or_default();
    let claude_env = agents::provider_environment_in_home("claude", &claude_cfg, &home);
    let codex_env = agents::provider_environment_in_home("codex", &codex_cfg, &home);
    let native = native_codex(app, uid, &home, settings).await;
    let mut login = login_file(&home);
    let mut codex = codex(
        &codex_cfg,
        &codex_env,
        &native,
        &login,
        std::env::var("CODEX_API_KEY").is_ok_and(|v| !v.is_empty()),
    );
    login["active"] = json!(codex["source"] == "login_file");
    // The internal classification is not a public credential field.
    login.as_object_mut().unwrap().remove("kind");
    codex["login_file"] = login;
    Ok(
        json!({"claude":claude(&claude_cfg, &claude_env, home.join(".claude/.credentials.json").is_file()),"codex":codex}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn env(values: &[(&str, &str)]) -> Env {
        values
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
    #[test]
    fn reports_effective_bearer_and_cloud_without_returning_secrets() {
        let cfg = AgentConfig {
            api_key: "saved-secret".into(),
            ..Default::default()
        };
        let result = claude(
            &cfg,
            &env(&[
                ("ANTHROPIC_AUTH_TOKEN", "host-secret"),
                ("ANTHROPIC_BASE_URL", "https://example.invalid"),
            ]),
            true,
        );
        assert_eq!(result["method"], "bearer_token");
        assert_eq!(result["provider"], "gateway");
        assert!(!result.to_string().contains("secret"));
        assert!(!result.to_string().contains("example.invalid"));
        let result = claude(
            &cfg,
            &env(&[
                ("CLAUDE_CODE_USE_BEDROCK", "1"),
                ("AWS_BEARER_TOKEN_BEDROCK", "secret"),
            ]),
            true,
        );
        assert_eq!(result["provider"], "bedrock");
        assert_eq!(result["source"], "AWS_BEARER_TOKEN_BEDROCK");
    }
    #[test]
    fn codex_login_file_is_bypassed_by_keys_and_native_aws_profiles() {
        let cfg = AgentConfig::default();
        let native = json!({"provider":"openai"});
        let login = json!({"kind":"chatgpt"});
        assert_eq!(
            codex(&cfg, &Env::new(), &native, &login, false)["method"],
            "chatgpt_login"
        );
        assert_eq!(
            codex(
                &cfg,
                &env(&[("CODEX_API_KEY", "secret")]),
                &native,
                &login,
                false
            )["source"],
            "OPENAI_API_KEY"
        );
        assert_eq!(
            codex(
                &cfg,
                &env(&[("CODEX_API_KEY", "secret")]),
                &native,
                &login,
                true
            )["source"],
            "CODEX_API_KEY"
        );
        let aws = json!({"provider":"amazon-bedrock-runtime","aws":true});
        assert_eq!(
            codex(&cfg, &Env::new(), &aws, &login, false)["method"],
            "aws_credentials"
        );
    }
    #[test]
    fn file_metadata_is_scoped_and_does_not_return_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::create_dir(home.join(".codex")).unwrap();
        assert_eq!(login_file(home)["exists"], false);
        std::fs::write(
            home.join(".codex/auth.json"),
            r#"{"tokens":{"access_token":"never-return-me"}}"#,
        )
        .unwrap();
        let metadata = login_file(home);
        assert_eq!(metadata["kind"], "chatgpt");
        assert!(!metadata.to_string().contains("never-return-me"));
        assert_eq!(login_file(&home.join("other-user"))["exists"], false);
    }
    #[tokio::test]
    async fn native_profiles_are_reported_without_disclosing_config_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let home = agents::user_home(&app, 1).unwrap();
        let config = home.join(".codex/config.toml");
        std::fs::write(&config, "model_provider = \"amazon-bedrock-runtime\"\n[model_providers.amazon-bedrock-runtime.aws]\nregion = \"eu-central-1\"\n").unwrap();
        std::fs::write(
            home.join(".codex/auth.json"),
            r#"{"tokens":{"access_token":"private-native-secret"}}"#,
        )
        .unwrap();
        let result = describe(&app, 1, &Settings::default()).await.unwrap();
        assert_eq!(result["codex"]["method"], "aws_credentials");
        assert_eq!(result["codex"]["provider"], "amazon-bedrock-runtime");
        assert_eq!(result["codex"]["login_file"]["active"], false);
        assert!(!result.to_string().contains("private-native-secret"));
        std::fs::write(config, "invalid = \"private-config-secret").unwrap();
        let result = describe(&app, 1, &Settings::default()).await.unwrap();
        assert_eq!(result["codex"]["method"], "configuration_error");
        assert!(!result.to_string().contains("private-config-secret"));
    }
}
