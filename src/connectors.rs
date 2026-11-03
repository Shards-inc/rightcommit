use crate::{App, agents, model::*};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::path::Path;

pub fn is_write(n: &Node) -> bool {
    (n.provider == "github" && n.operation == "api_request" && !["GET", "HEAD", "OPTIONS"].contains(&n.api_method.trim().to_ascii_uppercase().as_str()))
        || n.provider == "atlassian"
        || (n.provider != "local" && ["create_pr", "update_ticket", "apply_ticket_plan", "apply_ticket_batch", "create_incident", "order_catalog", "resolve_ticket"].contains(&n.operation.as_str()))
}
pub fn effect_digest(n: &Node, r: &Run, s: &Settings) -> String {
    // Supervising a request must not invalidate the exact effect being approved.
    let sessions: std::collections::BTreeMap<_, _> = r
        .sessions
        .iter()
        .filter(|(binding, _)| binding.as_str() != crate::supervision::BINDING)
        .collect();
    crate::store::hash(serde_json::to_vec(&json!({"provider":n.provider,"operation":n.operation,"node":n.id,"location":n.location,"command":n.command,"api_request":(&n.api_method,&n.api_path,&n.api_parameters,&n.api_query,&n.api_headers,&n.api_body,&n.api_encoding),"node_target":(&n.organization,&n.project,&n.repository,&n.target),"target":s.connections.get(&n.provider).map(|c|(&c.base_url,&c.project,&c.repository)),"run":r.id,"outputs":r.outputs,"sessions":sessions,"servicenow_auth":s.connections.get("servicenow").map(|c|(&c.authentication,&c.username,crate::store::hash(serde_json::to_vec(&(&c.token,&c.password,&c.client_id,&c.client_secret,&c.oauth_scope)).unwrap())))})).unwrap())
}
pub(crate) fn node_config(n: &Node, saved: &ConnectionConfig) -> Result<ConnectionConfig> {
    let mut c = saved.clone();
    if !n.repository.trim().is_empty() { c.repository = n.repository.trim().into(); }
    else if !n.target.is_empty() { c.repository = n.target.clone(); }
    if !n.project.trim().is_empty() { c.project = n.project.trim().into(); }
    if n.provider == "azure" && !n.organization.trim().is_empty() {
        c.base_url = format!("https://dev.azure.com/{}", segment(n.organization.trim())?);
    }
    if n.provider == "github" && !n.organization.trim().is_empty() && !c.repository.contains('/') {
        c.repository = format!("{}/{}", segment(n.organization.trim())?, c.repository);
    }
    if n.provider == "github" && c.base_url.is_empty() { c.base_url = "https://api.github.com".into(); }
    Ok(c)
}
fn segment(s: &str) -> Result<&str> {
    ensure!(
        !s.is_empty()
            && s.len() < 180
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)),
        "Identificativo remoto non valido"
    );
    Ok(s)
}
pub(crate) fn base(c: &ConnectionConfig) -> Result<String> {
    let u = reqwest::Url::parse(&c.base_url)?;
    let host = u.host_str().context("Host mancante")?;
    let extra = std::env::var("NODO_CONNECTOR_HOSTS").unwrap_or_default();
    ensure!(
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && (host == "api.github.com"
                || host == "dev.azure.com"
                || host.ends_with(".service-now.com")
                || extra.split(',').any(|x| x.trim() == host)),
        "Host connettore non consentito. Usa HTTPS e un host supportato, oppure NODO_CONNECTOR_HOSTS."
    );
    Ok(c.base_url.trim_end_matches('/').into())
}
pub async fn execute(app: &App, uid: i64, r: &Run, n: &Node) -> Result<AgentResult> {
    if n.provider == "servicenow" {
        return crate::servicenow::execute(app, uid, r, n).await;
    }
    if n.provider == "atlassian" {
        return crate::twg::execute(app, uid, r, n).await;
    }
    let cwd = Path::new(&r.workspace);
    if n.provider == "local" {
        let summary = match n.operation.as_str() {
            "read_ticket" => {
                let p = std::fs::canonicalize(cwd.join(&n.location))?;
                ensure!(
                    p.starts_with(cwd) && !cwd.join(&n.location).is_symlink(),
                    "Percorso ticket fuori dal workspace"
                );
                std::fs::read_to_string(p)?
            }
            "test" => agents::tool(app, uid, cwd, "cargo", &["test"], 600).await?,
            "commit" => {
                let status =
                    agents::tool(app, uid, cwd, "git", &["status", "--porcelain"], 30).await?;
                if !status.trim().is_empty() {
                    agents::tool(app, uid, cwd, "git", &["add", "--all"], 30).await?;
                    agents::tool(
                        app,
                        uid,
                        cwd,
                        "git",
                        &[
                            "-c",
                            "core.hooksPath=/dev/null",
                            "commit",
                            "-m",
                            "SDD: verified workflow delivery",
                        ],
                        30,
                    )
                    .await?;
                }
                let head = agents::tool(app, uid, cwd, "git", &["rev-parse", "HEAD"], 30).await?;
                format!(
                    "Commit locale verificato: {}. Nessun push, merge o deploy eseguito.",
                    head.trim()
                )
            }
            _ => bail!("Operazione locale sconosciuta"),
        };
        return Ok(AgentResult {
            status: "completed".into(),
            summary,
            decision: "pass".into(),
            ..Default::default()
        });
    }
    let s = app.store.settings(uid)?;
    let c = s
        .connections
        .get(&n.provider)
        .context("Configura il connettore nelle impostazioni personali")?;
    let c = &node_config(n, c)?;
    ensure!(!c.token.is_empty(), "Token connettore mancante");
    if n.provider == "github" && n.operation == "api_request" {
        return crate::github_api::execute(n, c, &s).await;
    }
    let b = base(c)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let repo = if !c.repository.is_empty() {
        &c.repository
    } else {
        &n.target
    };
    let branch = agents::tool(app, uid, cwd, "git", &["branch", "--show-current"], 30).await?;
    let report = format!(
        "rightcommit run {}: {}",
        r.id,
        r.outputs
            .values()
            .map(|o| o.summary.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let request=match(n.provider.as_str(),n.operation.as_str()){
        ("github","read_ticket")=>{let parts:Vec<_>=repo.split('/').collect();ensure!(parts.len()==2,"GitHub repository deve essere owner/repo");client.get(format!("{b}/repos/{}/{}/issues/{}",segment(parts[0])?,segment(parts[1])?,segment(&n.location)?))},
        ("github","read_ci")=>{let parts:Vec<_>=repo.split('/').collect();ensure!(parts.len()==2,"GitHub repository deve essere owner/repo");let head=agents::tool(app,uid,cwd,"git",&["rev-parse","HEAD"],30).await?;client.get(format!("{b}/repos/{}/{}/commits/{}/check-runs?per_page=100",segment(parts[0])?,segment(parts[1])?,head.trim()))},
        ("github","create_pr")=>{let parts:Vec<_>=repo.split('/').collect();ensure!(parts.len()==2,"GitHub repository deve essere owner/repo");
            // Reconcile by branch before POST, including retries after an uncertain response.
            let url=format!("{b}/repos/{}/{}/pulls",segment(parts[0])?,segment(parts[1])?);
            let old:Value=client.get(&url).bearer_auth(&c.token).header("User-Agent","rightcommit").query(&[("head",format!("{}:{}",parts[0],branch.trim())),("state","open".into())]).send().await?.error_for_status()?.json().await?;
            if let Some(v)=old.as_array().and_then(|a|a.first()){return Ok(AgentResult{status:"completed".into(),summary:serde_json::to_string_pretty(v)?,..Default::default()})}
            client.post(url).json(&json!({"title":r.workflow.name,"head":branch.trim(),"base":if n.location.is_empty(){"main"}else{&n.location},"body":report,"draft":true}))},
        ("azure","read_ticket")=>client.get(format!("{b}/{}/_apis/wit/workitems/{}?api-version=7.1",segment(&c.project)?,segment(&n.location)?)),
        ("azure","create_pr")=>client.post(format!("{b}/{}/_apis/git/repositories/{}/pullrequests?api-version=7.1",segment(&c.project)?,segment(repo)?)).json(&json!({"sourceRefName":format!("refs/heads/{}",branch.trim()),"targetRefName":format!("refs/heads/{}",if n.location.is_empty(){"main"}else{&n.location}),"title":r.workflow.name,"description":report,"isDraft":true})),
        ("servicenow","read_ticket")=>client.get(format!("{b}/api/now/table/incident/{}",segment(&n.location)?)),
        ("servicenow","update_ticket")=>client.patch(format!("{b}/api/now/table/incident/{}",segment(&n.location)?)).json(&json!({"work_notes":report})),
        _=>bail!("Operazione non supportata da questo connettore")
    };
    let request = if n.provider == "azure" {
        request.basic_auth("", Some(&c.token))
    } else {
        request.bearer_auth(&c.token)
    };
    let resp = request
        .header("User-Agent", "rightcommit/0.1")
        .send()
        .await?;
    let status = resp.status();
    let body: Value = resp.json().await?;
    ensure!(
        status.is_success(),
        "Connettore {} HTTP {}: {}",
        n.provider,
        status,
        agents::redact(&body.to_string(), &s)
            .chars()
            .take(1000)
            .collect::<String>()
    );
    if n.operation == "read_ci" {
        validate_ci(&body)?;
    }
    Ok(AgentResult {
        status: "completed".into(),
        summary: agents::redact(&serde_json::to_string_pretty(&body)?, &s),
        decision: "pass".into(),
        ..Default::default()
    })
}
pub(crate) fn validate_ci(body: &Value) -> Result<()> {
    let checks = body["check_runs"]
        .as_array()
        .context("Risposta CI senza check_runs")?;
    ensure!(
        body["total_count"].as_u64() == Some(checks.len() as u64),
        "Risposta CI paginata o incompleta: nessun via libera implicito"
    );
    ensure!(
        !checks.is_empty()
            && checks
                .iter()
                .all(|c| c["status"] == "completed" && c["conclusion"] == "success"),
        "CI non completa o non positiva per HEAD corrente"
    );
    Ok(())
}

#[cfg(test)]
mod node_target_tests {
    use super::*;
    #[test]
    fn node_targets_override_personal_defaults() {
        let saved = ConnectionConfig { base_url: "https://dev.azure.com/old".into(), project: "old".into(), repository: "old".into(), token: "secret".into(), ..Default::default() };
        let node = Node { provider: "azure".into(), organization: "team".into(), project: "project".into(), repository: "repo".into(), ..Default::default() };
        let c = node_config(&node, &saved).unwrap();
        assert_eq!((&*c.base_url, &*c.project, &*c.repository, &*c.token), ("https://dev.azure.com/team", "project", "repo", "secret"));
        assert!(node_config(&Node { organization: "bad/path".into(), ..node }, &saved).is_err());
    }
    #[test]
    fn github_owner_and_legacy_targets() {
        let node = Node { provider: "github".into(), organization: "owner".into(), repository: "repo".into(), ..Default::default() };
        let c = node_config(&node, &ConnectionConfig::default()).unwrap();
        assert_eq!(c.repository, "owner/repo");
        assert_eq!(c.base_url, "https://api.github.com");
        let saved = ConnectionConfig { repository: "legacy/repo".into(), ..Default::default() };
        assert_eq!(node_config(&Node::default(), &saved).unwrap().repository, "legacy/repo");
        assert_eq!(node_config(&Node { target: "workflow/repo".into(), ..Default::default() }, &saved).unwrap().repository, "workflow/repo");
    }
}
