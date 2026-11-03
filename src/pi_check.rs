use crate::{
    App, agents,
    auth::{ApiResult, bad},
    model::{AgentConfig, User},
};
use axum::{Json, extract::State};
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::io::AsyncReadExt;

/// An isolated working directory; remove it after the probe process exits.
struct ProbeWorkspace(std::path::PathBuf);
impl Drop for ProbeWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn draft_config(mut draft: AgentConfig, saved: &AgentConfig) -> anyhow::Result<AgentConfig> {
    draft.model = draft.model.trim().into();
    anyhow::ensure!(
        !draft.model.is_empty(),
        "Scegli un modello Pi prima di verificare la connessione."
    );
    anyhow::ensure!(
        crate::pi_providers::known(&draft.provider),
        "Provider Pi non valido."
    );
    anyhow::ensure!(
        draft.model.len() <= 200 && draft.api_key.len() <= 4096,
        "Configurazione Pi non valida."
    );
    if draft.multi_provider {
        draft.multi_provider = true;
        draft.api_key.clear();
    } else if draft.api_key == "__CLEAR__" {
        draft.api_key.clear();
    } else if draft.api_key.is_empty()
        && !saved.multi_provider
        && draft.provider == saved.provider
        && (draft.provider != "local"
            || draft.base_url.trim().trim_end_matches('/')
                == saved.base_url.trim().trim_end_matches('/'))
    {
        draft.api_key = saved.api_key.clone();
    }
    if draft.provider == "local" && draft.base_url.trim().is_empty() {
        draft.base_url = std::env::var("PI_LOCAL_BASE_URL").unwrap_or_default();
    }
    crate::pi_local::validate(&draft)?;
    Ok(draft)
}

fn failure_message(output: &str) -> &'static str {
    let detail = output.to_lowercase();
    if detail.contains("401")
        || detail.contains("403")
        || detail.contains("unauthorized")
        || detail.contains("invalid api key")
        || detail.contains("no api key")
        || detail.contains("authentication")
    {
        "Autenticazione Pi fallita. Verifica la chiave API del provider selezionato."
    } else if detail.contains("429")
        || detail.contains("quota")
        || detail.contains("credit")
        || detail.contains("rate limit")
    {
        "Il provider Pi ha raggiunto un limite o non dispone di credito. Riprova più tardi o verifica il tuo account."
    } else if detail.contains("model")
        && (detail.contains("not found")
            || detail.contains("unknown")
            || detail.contains("not available")
            || detail.contains("not support"))
    {
        "Il modello Pi non è disponibile. Verifica il modello e i permessi del provider selezionato."
    } else {
        "Verifica Pi fallita. Controlla provider, modello, credenziali e disponibilità della rete."
    }
}

fn check_output(stdout: &[u8], stderr: &[u8], success: bool) -> Result<(), &'static str> {
    let output = String::from_utf8_lossy(stdout);
    let mut replied = false;
    let mut failed = !success;
    for line in output.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if event["type"] == "error" {
            failed = true;
        }
        if event["type"] == "message_end" && event["message"]["role"] == "assistant" {
            let message = &event["message"];
            if matches!(
                message["stopReason"].as_str(),
                Some("error" | "aborted" | "toolUse")
            ) {
                failed = true;
            }
            replied |= message["content"].as_array().is_some_and(|content| {
                content.iter().any(|item| {
                    item["type"] == "text"
                        && item["text"]
                            .as_str()
                            .is_some_and(|text| !text.trim().is_empty())
                })
            });
        }
    }
    if failed || !replied {
        // Never expose native output: it can include credentials or authentication headers.
        return Err(failure_message(&format!(
            "{output}\n{}",
            String::from_utf8_lossy(stderr)
        )));
    }
    Ok(())
}

pub async fn check(
    State(app): State<App>,
    user: User,
    Json(draft): Json<AgentConfig>,
) -> ApiResult<Json<Value>> {
    let mut settings = app.store.settings(user.id)?;
    let saved = settings.agents.get("pi").cloned().unwrap_or_default();
    let config = draft_config(draft, &saved)?;
    if settings
        .connections
        .get("servicenow")
        .is_some_and(|sn| [&sn.password, &sn.client_secret, &sn.token].into_iter().any(|secret| !secret.is_empty() && secret == &config.api_key))
    {
        return Err(bad(
            "Una credenziale ServiceNow è finita nei campi di un agente. Salvataggio bloccato: controlla l’autocompilazione del browser o del gestore password.",
        ));
    }
    let provider = config.provider.clone();
    let model = config.model.clone();
    settings.agents.insert("pi".into(), config);
    let home = agents::user_home(&app, user.id)?;
    let path = home
        .join("tmp")
        .join(format!("pi-check-{}", crate::model::id()));
    std::fs::create_dir(&path)?;
    let workspace = ProbeWorkspace(path);
    crate::store::private(&workspace.0, true)?;
    agents::own(&app, user.id, &workspace.0)?;
    let mut command = agents::command(&app, user.id, "pi", &workspace.0, &settings)?;
    crate::pi_local::configure(&app, user.id, &settings, &model, &mut command)?;
    command
        .args([
            "--mode",
            "json",
            "--print",
            "--no-session",
            "--no-tools",
            "--no-extensions",
            "--no-mcp",
            "--no-skills",
            "--no-prompt-templates",
            "--no-themes",
            "--no-context-files",
            "--no-approve",
            "--offline",
            "--thinking",
            "off",
            "--provider",
            &provider,
            "--model",
            &model,
            "--system-prompt",
            "Connection check. Reply with OK only.",
            "--",
            "Reply with OK only.",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|_| bad("Impossibile avviare Pi. Verifica che sia installato nell’app."))?;
    let _group = agents::ProcessGroup(
        child
            .id()
            .ok_or_else(|| bad("Impossibile avviare Pi. Verifica che sia installato nell’app."))?,
    );
    let mut stdout = child.stdout.take().unwrap().take(65537);
    let mut stderr = child.stderr.take().unwrap().take(65537);
    let result = tokio::time::timeout(Duration::from_secs(45), async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (status, read_out, read_err) = tokio::join!(child.wait(), stdout.read_to_end(&mut out), stderr.read_to_end(&mut err));
        read_out?; read_err?;
        Ok::<_, std::io::Error>((status?, out, err))
    }).await.map_err(|_|bad("La verifica Pi ha superato il tempo massimo. Riprova tra poco."))?
        .map_err(|_|bad("Verifica Pi fallita. Controlla provider, modello, credenziali e disponibilità della rete."))?;
    if result.1.len() > 65536 || result.2.len() > 65536 {
        return Err(bad("Risposta di verifica Pi non valida."));
    }
    check_output(&result.1, &result.2, result.0.success()).map_err(bad)?;
    Ok(Json(
        json!({"connected":true,"message":"Connessione Pi riuscita. Il modello ha risposto correttamente."}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_check_uses_draft_without_saving_and_redacts_failures() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        app.store
            .update_settings(1, |settings| {
                settings.agents.get_mut("pi").unwrap().api_key = "original-secret".into();
                Ok(())
            })
            .unwrap();
        let original = serde_json::to_value(app.store.settings(1).unwrap()).unwrap();
        let tools = crate::distribution::tools(dir.path());
        std::fs::create_dir_all(&tools).unwrap();
        let binary = tools.join("pi");
        std::fs::write(&binary, r##"#!/bin/sh
[ "$OPENAI_API_KEY" = "draft-key" ] || exit 9
for flag in --no-tools --no-session --no-extensions --no-mcp --no-context-files; do
  case " $* " in *" $flag "*) ;; *) exit 10 ;; esac
done
printf '%s\n' '{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"text","text":"OK"}]}}'
"##).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let user = || User {
            id: 1,
            email: "check@test.local".into(),
        };
        let draft = || AgentConfig {
            provider: "openai".into(),
            model: "draft-model".into(),
            api_key: "draft-key".into(),
            ..Default::default()
        };
        assert_eq!(
            check(State(app.clone()), user(), Json(draft()))
                .await
                .unwrap()
                .0["connected"],
            true
        );
        assert_eq!(
            serde_json::to_value(app.store.settings(1).unwrap()).unwrap(),
            original
        );
        std::fs::write(
            &binary,
            "#!/bin/sh\necho '401 draft-key secret-header' >&2\nexit 1\n",
        )
        .unwrap();
        let error = check(State(app.clone()), user(), Json(draft()))
            .await
            .unwrap_err();
        assert!(error.1.contains("Autenticazione"));
        assert!(!error.1.contains("draft-key"));
        assert!(!error.1.contains("secret-header"));
        assert_eq!(
            serde_json::to_value(app.store.settings(1).unwrap()).unwrap(),
            original
        );
    }
    #[test]
    fn draft_credentials_are_scoped_to_provider() {
        let saved = AgentConfig {
            api_key: "saved-key".into(),
            ..Default::default()
        };
        let mut draft = AgentConfig {
            model: "chosen-model".into(),
            ..Default::default()
        };
        assert_eq!(
            draft_config(draft.clone(), &saved).unwrap().api_key,
            "saved-key"
        );
        draft.provider = "openai".into();
        assert!(
            draft_config(draft.clone(), &saved)
                .unwrap()
                .api_key
                .is_empty()
        );
        draft.api_key = "draft-key".into();
        assert_eq!(
            draft_config(draft.clone(), &saved).unwrap().api_key,
            "draft-key"
        );
        draft.api_key = "__CLEAR__".into();
        assert!(
            draft_config(draft.clone(), &saved)
                .unwrap()
                .api_key
                .is_empty()
        );
        draft.model.clear();
        assert!(draft_config(draft, &saved).is_err());
    }
    #[test]
    fn response_requires_successful_assistant_and_never_exposes_output() {
        let success=br#"{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"text","text":"OK"}]}}"#;
        assert!(check_output(success, b"", true).is_ok());
        assert!(check_output(success, b"", false).is_err());
        assert!(check_output(b"{}", b"", true).is_err());
        let error=br#"{"type":"message_end","message":{"role":"assistant","stopReason":"error","content":[],"errorMessage":"401 secret-credential"}}"#;
        let result = check_output(error, b"", true).unwrap_err();
        assert!(result.contains("Autenticazione"));
        assert!(!result.contains("secret-credential"));
    }
}
