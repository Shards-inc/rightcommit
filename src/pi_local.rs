use crate::{
    App, agents,
    model::{AgentConfig, Settings},
};
use anyhow::{Result, ensure};
use serde_json::json;
use tokio::process::Command;

pub(crate) fn validate(config: &AgentConfig) -> Result<()> {
    if config.provider != "local" {
        return Ok(());
    }
    ensure!(
        config.base_url.len() <= 2048,
        "URL del provider locale non valido. Usa un URL HTTP o HTTPS senza credenziali, query o frammenti."
    );
    let address = if config.base_url.trim().is_empty() {
        std::env::var("PI_LOCAL_BASE_URL").unwrap_or_default()
    } else {
        config.base_url.clone()
    };
    let url = reqwest::Url::parse(address.trim()).ok();
    ensure!(
        url.is_some_and(|url| matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()),
        "URL del provider locale non valido. Usa un URL HTTP o HTTPS senza credenziali, query o frammenti."
    );
    Ok(())
}

pub(crate) fn endpoint_changed(config: &AgentConfig, previous: &AgentConfig) -> bool {
    (config.provider == "local" || previous.provider == "local")
        && (config.provider != previous.provider
            || config.base_url.trim().trim_end_matches('/')
                != previous.base_url.trim().trim_end_matches('/'))
}

/// Register only this request's endpoint/model. No shared models.json or cloud credentials change.
pub(crate) fn configure(
    app: &App,
    uid: i64,
    settings: &Settings,
    model: &str,
    command: &mut Command,
) -> Result<()> {
    let mut config = settings.agents.get("pi").cloned().unwrap_or_default();
    if config.provider != "local" {
        return Ok(());
    }
    if config.base_url.trim().is_empty() {
        config.base_url = std::env::var("PI_LOCAL_BASE_URL").unwrap_or_default();
    }
    validate(&config)?;
    agents::validate_pi_model("pi", model)?;
    let context = settings
        .compaction
        .get("pi")
        .map(|cfg| cfg.context_window_tokens)
        .filter(|n| *n > 0)
        .unwrap_or(32768);
    let definition = json!({
        "name":"Local / OpenAI compatible", "baseUrl":config.base_url.trim().trim_end_matches('/'),
        "api":"openai-completions", "apiKey":"$PI_LOCAL_API_KEY",
        "models":[{"id":model,"name":model,"reasoning":false,"input":["text"],
            "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},
            "contextWindow":context,"maxTokens":(context/4).clamp(1,8192)}]
    });
    let source =
        format!("export default function(pi) {{ pi.registerProvider('local', {definition}); }}\n");
    let dir = agents::user_home(app, uid)?.join(".pi/agent/rightcommit-providers");
    ensure!(!dir.is_symlink(), "Directory provider Pi non valida");
    std::fs::create_dir_all(&dir)?;
    crate::store::private(&dir, true)?;
    agents::own(app, uid, &dir)?;
    let path = dir.join(format!("{}.mjs", crate::store::hash(source.as_bytes())));
    let temporary = dir.join(format!("{}.tmp", crate::model::id()));
    let result = (|| -> Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(source.as_bytes())?;
        agents::own(app, uid, &temporary)?;
        std::fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result?;
    command.arg("--extension").arg(path);
    let key = agents::provider_environment("pi", &config)
        .remove("PI_LOCAL_API_KEY")
        .filter(|key| !key.trim().is_empty())
        .unwrap_or_else(|| "local".into());
    command.env("PI_LOCAL_API_KEY", key);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_local_servers_but_rejects_embedded_credentials_and_invalid_urls() {
        for url in [
            "http://127.0.0.1:11434/v1",
            "http://host.docker.internal:1234/v1/",
            "https://inference.example/v1",
        ] {
            assert!(
                validate(&AgentConfig {
                    provider: "local".into(),
                    base_url: url.into(),
                    ..Default::default()
                })
                .is_ok()
            );
        }
        for url in [
            "",
            "file:///tmp/model",
            "ftp://server/v1",
            "http://user:secret@server/v1",
            "http://server/v1?key=secret",
            "http://server/v1#secret",
        ] {
            assert!(
                validate(&AgentConfig {
                    provider: "local".into(),
                    base_url: url.into(),
                    ..Default::default()
                })
                .is_err()
            );
        }
    }
    #[test]
    fn local_configurations_are_isolated_and_keep_keys_out_of_generated_files_and_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let mut settings = Settings::default();
        settings.agents.insert(
            "pi".into(),
            AgentConfig {
                provider: "local".into(),
                base_url: "http://127.0.0.1:1234/v1".into(),
                api_key: "local-private-key".into(),
                ..Default::default()
            },
        );
        let mut first = Command::new("pi");
        configure(&app, 1, &settings, "model-a", &mut first).unwrap();
        let mut second = Command::new("pi");
        configure(&app, 1, &settings, "model-b", &mut second).unwrap();
        let path = |command: &Command| command.as_std().get_args().last().unwrap().to_os_string();
        assert_ne!(path(&first), path(&second));
        let source = std::fs::read_to_string(path(&first)).unwrap();
        assert!(source.contains("model-a"));
        assert!(!source.contains("model-b"));
        assert!(!source.contains("local-private-key"));
        assert!(!format!("{:?}", first.as_std().get_args()).contains("local-private-key"));
        assert!(
            first
                .as_std()
                .get_envs()
                .any(|(key, value)| key == "PI_LOCAL_API_KEY"
                    && value == Some(std::ffi::OsStr::new("local-private-key")))
        );
        settings.agents.get_mut("pi").unwrap().api_key.clear();
        let mut keyless = Command::new("pi");
        configure(&app, 1, &settings, "model-a", &mut keyless).unwrap();
        assert!(
            keyless
                .as_std()
                .get_envs()
                .any(|(key, value)| key == "PI_LOCAL_API_KEY"
                    && value.is_some_and(|v| !v.is_empty()))
        );
    }
}
