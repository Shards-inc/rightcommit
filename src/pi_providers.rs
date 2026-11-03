use crate::{
    App, agents,
    model::{AgentConfig, Settings},
};
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path, sync::OnceLock};

#[derive(Deserialize)]
pub(crate) struct Provider {
    pub id: String,
    pub label: String,
    pub key_env: String,
    pub env: Vec<String>,
}
#[derive(Deserialize)]
struct Catalog {
    providers: Vec<Provider>,
}
pub(crate) fn catalog() -> &'static [Provider] {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    &CATALOG
        .get_or_init(|| {
            serde_json::from_str(include_str!("../distribution/pi-providers.json")).unwrap()
        })
        .providers
}
pub(crate) fn known(id: &str) -> bool {
    catalog().iter().any(|provider| provider.id == id)
}
pub(crate) fn public_catalog() -> Value {
    serde_json::from_str::<Value>(include_str!("../distribution/pi-providers.json")).unwrap()["providers"].clone()
}
pub(crate) fn key_env(id: &str) -> Option<&'static str> {
    catalog()
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.key_env.as_str())
        .filter(|key| !key.is_empty())
}
pub(crate) fn environment(
    config: &AgentConfig,
    lookup: impl Fn(&str) -> Option<String>,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    if let Some(provider) = catalog()
        .iter()
        .find(|provider| provider.id == config.provider)
    {
        for name in &provider.env {
            if let Some(value) = lookup(name).filter(|value| !value.is_empty()) {
                env.insert(name.clone(), value);
            }
        }
        if config.provider == "anthropic"
            && lookup("ANTHROPIC_BASE_URL").is_some_and(|value| !value.is_empty())
        {
            env.remove("ANTHROPIC_AUTH_TOKEN");
            env.remove("ANTHROPIC_OAUTH_TOKEN");
        }
        if !config.multi_provider && !config.api_key.is_empty() && !provider.key_env.is_empty() {
            // A personal key takes precedence over alternative environment tokens.
            if config.provider == "anthropic" {
                env.remove("ANTHROPIC_AUTH_TOKEN");
                env.remove("ANTHROPIC_OAUTH_TOKEN");
            }
            env.insert(provider.key_env.clone(), config.api_key.clone());
        }
    }
    env
}
pub(crate) fn effective(
    settings: &Settings,
    provider: &str,
    model: &str,
    base_url: &str,
) -> Result<AgentConfig> {
    let mut config = settings.agents.get("pi").cloned().unwrap_or_default();
    let default = config.provider.clone();
    if !provider.is_empty() {
        config.provider = provider.into();
    }
    if config.multi_provider || config.provider != default {
        config.api_key.clear();
    }
    if config.provider != default {
        config.model.clear();
        config.base_url.clear();
    }
    if !base_url.is_empty() {
        config.base_url = base_url.into();
    }
    ensure!(
        known(&config.provider),
        "Provider Pi non supportato: {}",
        config.provider
    );
    if !model.is_empty() {
        config.model = model.into();
    }
    if config.provider == "local" && config.base_url.trim().is_empty() {
        config.base_url = std::env::var("PI_LOCAL_BASE_URL").unwrap_or_default();
    }
    crate::pi_local::validate(&config)?;
    agents::validate_pi_model("pi", &config.model)?;
    Ok(config)
}
pub(crate) fn available(config: &AgentConfig, home: &Path) -> bool {
    let mut env = environment(config, |name| std::env::var(name).ok());
    let path = home.join(".pi/agent/auth.json");
    if !path.is_symlink()
        && std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() <= 1024 * 1024)
        && let Some(auth) = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        && let Some(native_env) = auth[&config.provider]["env"].as_object()
        && let Some(provider) = catalog()
            .iter()
            .find(|provider| provider.id == config.provider)
    {
        for name in &provider.env {
            if let Some(value) = native_env
                .get(name)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                env.insert(name.clone(), value.into());
            }
        }
    }
    let has = |name: &str| env.get(name).is_some_and(|value| !value.trim().is_empty());
    let native = agents::pi_has_login(home, &config.provider);
    match config.provider.as_str() {
        "local" => crate::pi_local::validate(config).is_ok(),
        "azure" => {
            (has("AZURE_OPENAI_API_KEY") || native)
                && (has("AZURE_OPENAI_BASE_URL") || has("AZURE_OPENAI_RESOURCE_NAME"))
        }
        "amazon-bedrock" => {
            native
                || has("AWS_BEARER_TOKEN_BEDROCK")
                || (has("AWS_ACCESS_KEY_ID") && has("AWS_SECRET_ACCESS_KEY"))
                || has("AWS_PROFILE")
                || has("AWS_SHARED_CREDENTIALS_FILE")
                || has("AWS_WEB_IDENTITY_TOKEN_FILE")
                || has("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI")
                || has("AWS_CONTAINER_CREDENTIALS_FULL_URI")
                || home.join(".aws/credentials").is_file()
        }
        "google-vertex" => {
            native
                || has("GOOGLE_CLOUD_API_KEY")
                || ((has("GOOGLE_APPLICATION_CREDENTIALS")
                    || home
                        .join(".config/gcloud/application_default_credentials.json")
                        .is_file())
                    && (has("GOOGLE_CLOUD_PROJECT") || has("GCLOUD_PROJECT"))
                    && has("GOOGLE_CLOUD_LOCATION"))
        }
        "cloudflare-workers-ai" => {
            (native || has("CLOUDFLARE_API_KEY")) && has("CLOUDFLARE_ACCOUNT_ID")
        }
        "cloudflare-ai-gateway" => {
            (native || has("CLOUDFLARE_API_KEY"))
                && has("CLOUDFLARE_ACCOUNT_ID")
                && has("CLOUDFLARE_GATEWAY_ID")
        }
        "anthropic" => {
            native
                || has("ANTHROPIC_API_KEY")
                || has("ANTHROPIC_AUTH_TOKEN")
                || has("ANTHROPIC_OAUTH_TOKEN")
                || (has("ANTHROPIC_FEDERATION_RULE_ID")
                    && has("ANTHROPIC_ORGANIZATION_ID")
                    && has("ANTHROPIC_IDENTITY_TOKEN_FILE"))
        }
        _ => native || key_env(&config.provider).is_some_and(has),
    }
}
pub(crate) fn any_available(settings: &Settings, home: &Path) -> bool {
    catalog().iter().any(|provider| {
        let mut config = settings.agents.get("pi").cloned().unwrap_or_default();
        config.multi_provider = true;
        config.api_key.clear();
        if config.provider != provider.id {
            config.base_url.clear();
        }
        config.provider = provider.id.clone();
        if config.provider == "local" && config.base_url.is_empty() {
            config.base_url = std::env::var("PI_LOCAL_BASE_URL").unwrap_or_default();
        }
        available(&config, home)
    })
}
pub(crate) fn preflight(
    app: &App,
    uid: i64,
    settings: &Settings,
    provider: &str,
    model: &str,
    url: &str,
) -> Result<AgentConfig> {
    let config = effective(settings, provider, model, url)?;
    let home = app.data.join("users").join(uid.to_string());
    ensure!(
        available(&config, &home),
        "Configurazione Pi mancante per {}. Configura le credenziali e i parametri del provider in .env o nel login nativo Pi.",
        catalog()
            .iter()
            .find(|provider| provider.id == config.provider)
            .map(|provider| provider.label.as_str())
            .unwrap_or(&config.provider)
    );
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_matches_installed_distribution_and_only_contains_chat_providers() {
        let version: Value =
            serde_json::from_str(include_str!("../distribution/pi-providers.json")).unwrap();
        assert!(
            include_str!("../distribution/agents.env").contains(&format!(
                "PI_VERSION={}",
                version["version"].as_str().unwrap()
            ))
        );
        assert!(catalog().len() > 30);
        assert!(known("openai-codex"));
        assert!(known("google-vertex"));
        assert!(known("local"));
        assert!(!known("typesafe"));
        assert_eq!(
            catalog()
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            catalog().len()
        );
    }
    #[test]
    fn multi_provider_ignores_generic_key_and_never_inherits_another_providers_model() {
        let mut settings = Settings::default();
        settings.agents.insert(
            "pi".into(),
            AgentConfig {
                multi_provider: true,
                provider: "openai".into(),
                model: "gpt-default".into(),
                api_key: "personal-key-must-not-leak".into(),
                ..Default::default()
            },
        );
        let selected = effective(&settings, "xai", "grok-model", "").unwrap();
        assert_eq!(selected.provider, "xai");
        assert_eq!(selected.model, "grok-model");
        assert!(selected.api_key.is_empty());
        assert!(effective(&settings, "xai", "", "").is_err());
        let default = effective(&settings, "", "", "").unwrap();
        assert_eq!(default.provider, "openai");
        assert_eq!(default.model, "gpt-default");
        assert!(default.api_key.is_empty());
        let local = effective(
            &settings,
            "local",
            "local-model",
            "http://localhost:1234/v1",
        )
        .unwrap();
        assert_eq!(local.base_url, "http://localhost:1234/v1");
        assert!(local.api_key.is_empty());
        assert!(effective(&settings, "unknown-provider", "model", "").is_err());
    }
    #[test]
    fn each_provider_receives_only_its_declared_environment() {
        let host = BTreeMap::from([
            ("OPENAI_API_KEY", "openai-key"),
            ("XAI_API_KEY", "xai-key"),
            ("GROQ_API_KEY", "groq-key"),
            ("AWS_ACCESS_KEY_ID", "aws-key"),
            ("AWS_SECRET_ACCESS_KEY", "aws-secret"),
            ("AWS_REGION", "region"),
            ("AZURE_OPENAI_API_KEY", "azure-key"),
            ("AZURE_OPENAI_BASE_URL", "https://azure.example"),
            ("PI_LOCAL_BASE_URL", "http://local/v1"),
            ("PI_LOCAL_API_KEY", "local-key"),
            ("UNRELATED_SECRET", "never"),
        ]);
        for (provider, expected) in [
            ("openai", vec!["OPENAI_API_KEY"]),
            ("xai", vec!["XAI_API_KEY"]),
            ("groq", vec!["GROQ_API_KEY"]),
            (
                "azure",
                vec!["AZURE_OPENAI_API_KEY", "AZURE_OPENAI_BASE_URL"],
            ),
            (
                "amazon-bedrock",
                vec!["AWS_ACCESS_KEY_ID", "AWS_REGION", "AWS_SECRET_ACCESS_KEY"],
            ),
            ("local", vec!["PI_LOCAL_API_KEY", "PI_LOCAL_BASE_URL"]),
        ] {
            let config = AgentConfig {
                provider: provider.into(),
                multi_provider: true,
                api_key: "ignored-key".into(),
                ..Default::default()
            };
            let env = environment(&config, |key| host.get(key).map(|value| value.to_string()));
            assert_eq!(env.keys().map(String::as_str).collect::<Vec<_>>(), expected);
        }
    }
    #[test]
    fn native_credentials_are_provider_scoped_and_missing_cloud_parameters_fail_preflight() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let home = agents::user_home(&app, 1).unwrap();
        std::fs::write(
            home.join(".pi/agent/auth.json"),
            r#"{"xai":{"type":"api_key","key":"native-xai"}}"#,
        )
        .unwrap();
        let mut settings = Settings::default();
        settings.agents.insert(
            "pi".into(),
            AgentConfig {
                multi_provider: true,
                provider: "openai".into(),
                ..Default::default()
            },
        );
        let result = preflight(&app, 1, &settings, "xai", "grok-model", "").unwrap();
        assert_eq!(result.provider, "xai");
        assert!(result.api_key.is_empty());
        let error = preflight(&app, 1, &settings, "azure", "azure-model", "")
            .unwrap_err()
            .to_string();
        assert!(error.contains("Configurazione Pi mancante"));
        assert!(!error.contains("native-xai"));
        std::fs::write(home.join(".pi/agent/auth.json"),r#"{"azure":{"type":"api_key","key":"native-azure","env":{"AZURE_OPENAI_BASE_URL":"https://azure.example"}}}"#).unwrap();
        assert_eq!(
            preflight(&app, 1, &settings, "azure", "azure-model", "")
                .unwrap()
                .provider,
            "azure"
        );
        let old: crate::model::NativeSession = serde_json::from_str(
            r#"{"runtime":"pi","native_id":"old","model":"old-model","turns":2}"#,
        )
        .unwrap();
        assert!(old.provider.is_empty());
        assert!(old.base_url.is_empty());
    }
}
