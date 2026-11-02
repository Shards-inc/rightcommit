use crate::{
    App,
    agents::{self, emit},
    connectors,
    model::*,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

pub fn edge_allowed(a: &Node, b: &Node, kind: &str, outcome: &str) -> bool {
    if a.kind == "Comment" || b.kind == "Comment" {
        return false;
    }
    match kind {
        "context" => {
            ["MCP", "RAG"].contains(&a.kind.as_str()) && b.kind == "Agent"
                || a.kind == "Source"
                    && ["Agent", "Connection", "Workflow", "A2A"].contains(&b.kind.as_str())
                || a.kind == "Agent" && b.kind == "Task" && b.agent == a.id
        }
        "control" => {
            !["Source", "Agent", "MCP", "RAG"].contains(&a.kind.as_str())
                && !["Source", "Agent", "MCP", "RAG"].contains(&b.kind.as_str())
        }
        "rework" => {
            (a.kind == "Gate" || a.kind == "Review")
                && ["Task", "Workflow"].contains(&b.kind.as_str())
                && outcome == "rework"
        }
        _ => false,
    }
}
// Parallel regions are deliberately structured: disjoint Task chains and one barrier.
fn parallel_branches(w: &Workflow, fork: &Node) -> Result<Vec<Vec<Node>>> {
    ensure!(
        w.node(&fork.target)?.kind == "Join",
        "Fork richiede un Join destinatario"
    );
    let starts: Vec<_> = w
        .edges
        .iter()
        .filter(|e| e.from == fork.id && e.kind == "control")
        .collect();
    ensure!(starts.len() >= 2, "Fork richiede almeno due rami");
    let mut seen = HashSet::new();
    let mut bindings = std::collections::HashMap::new();
    let mut branches = Vec::new();
    for (index, edge) in starts.iter().enumerate() {
        ensure!(edge.outcome.is_empty(), "I rami Fork non hanno esiti");
        let mut current = edge.to.clone();
        let mut branch = Vec::new();
        while current != fork.target {
            ensure!(
                seen.insert(current.clone()),
                "Rami paralleli sovrapposti o ciclici"
            );
            let node = w.node(&current)?;
            ensure!(
                node.kind == "Task",
                "I rami paralleli supportano sequenze di Task"
            );
            if let Some(owner) = bindings.insert(node.agent.clone(), index) {
                ensure!(
                    owner == index,
                    "Ogni ramo parallelo richiede Agent distinti"
                );
            }
            ensure!(
                w.edges
                    .iter()
                    .filter(|e| e.to == current && e.kind != "context")
                    .count()
                    == 1,
                "Ingresso esterno in un ramo parallelo"
            );
            let next: Vec<_> = w
                .edges
                .iter()
                .filter(|e| e.from == current && e.kind != "context")
                .collect();
            ensure!(
                next.len() == 1 && next[0].kind == "control" && next[0].outcome.is_empty(),
                "Ogni Task parallelo deve proseguire fino al Join"
            );
            branch.push(node.clone());
            current = next[0].to.clone();
        }
        ensure!(!branch.is_empty(), "Ramo parallelo vuoto");
        branches.push(branch);
    }
    ensure!(
        w.edges
            .iter()
            .filter(|e| e.to == fork.target && e.kind != "context")
            .all(|e| seen.contains(&e.from)),
        "Join con ingressi estranei al Fork"
    );
    Ok(branches)
}
fn validate_parallel(w: &Workflow) -> Result<()> {
    let mut joins = HashSet::new();
    for fork in w.nodes.iter().filter(|n| n.kind == "Fork") {
        ensure!(
            joins.insert(fork.target.clone()),
            "Un Join può appartenere a un solo Fork"
        );
        parallel_branches(w, fork)?;
    }
    for join in w.nodes.iter().filter(|n| n.kind == "Join") {
        ensure!(
            joins.contains(&join.id) && w.entry != join.id,
            "Join senza Fork"
        );
        ensure!(
            w.edges
                .iter()
                .filter(|e| e.from == join.id && e.kind == "control")
                .count()
                <= 1,
            "Join con più continuazioni"
        );
    }
    Ok(())
}

pub fn validate_draft(w: &Workflow) -> Result<()> {
    ensure!(
        !w.name.trim().is_empty()
            && w.name.len() < 160
            && w.nodes.len() <= 200
            && w.edges.len() <= 600,
        "Dimensioni o nome della bozza non validi"
    );
    let mut ids = HashSet::new();
    for n in &w.nodes {
        ensure!(
            !n.id.is_empty()
                && n.id.len() < 80
                && n.id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                && ids.insert(&n.id),
            "ID nodo non valido o duplicato"
        );
        ensure!(
            NODE_KINDS.contains(&n.kind.as_str()),
            "Tipo nodo non disponibile: {}",
            n.kind
        );
        ensure!(
            n.title.len() < 180
                && n.prompt.len() < 64000
                && n.text.len() <= 64000
                && n.x.is_finite()
                && n.y.is_finite()
                && n.width.is_finite()
                && n.height.is_finite(),
            "Configurazione nodo non valida"
        );
        if n.kind == "Comment" {
            ensure!(
                (n.width == 0.0 || (220.0..=1200.0).contains(&n.width))
                    && (n.height == 0.0 || (160.0..=1200.0).contains(&n.height)),
                "Dimensioni commento non valide"
            );
        }
        if !n.agent.is_empty() && ["Task", "Gate", "Review"].contains(&n.kind.as_str()) {
            ensure!(
                w.node(&n.agent)?.kind == "Agent",
                "Associazione Agent non valida"
            );
        }
    }
    if !w.entry.is_empty() {
        ensure!(
            !["Source", "Agent", "MCP", "RAG", "Comment"]
                .contains(&w.node(&w.entry)?.kind.as_str()),
            "Ingresso non eseguibile"
        );
    }
    let mut edges = HashSet::new();
    for e in &w.edges {
        let a = w.node(&e.from)?;
        let b = w.node(&e.to)?;
        ensure!(
            a.id != b.id
                && edges.insert((&e.from, &e.to, &e.kind, &e.outcome))
                && edge_allowed(a, b, &e.kind, &e.outcome),
            "Collegamento non valido: {} → {} ({})",
            e.from,
            e.to,
            e.kind
        );
    }
    Ok(())
}
pub fn validate(w: &Workflow) -> Result<()> {
    validate_draft(w)?;
    crate::supervision::validate(&w.execution)?;
    validate_parallel(w)?;
    ensure!(
        !w.name.trim().is_empty() && w.name.len() < 160,
        "Nome workflow non valido"
    );
    ensure!(
        !w.nodes.is_empty() && w.nodes.len() <= 200 && w.edges.len() <= 600,
        "Il workflow richiede 1–200 nodi"
    );
    let mut ids = HashSet::new();
    for n in &w.nodes {
        ensure!(
            !n.id.is_empty()
                && n.id.len() < 80
                && n.id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                && ids.insert(&n.id),
            "ID nodo non valido o duplicato"
        );
        ensure!(
            NODE_KINDS.contains(&n.kind.as_str()),
            "Tipo nodo non valido"
        );
        ensure!(
            n.prompt.len() < 64000 && n.title.len() < 180 && n.x.is_finite() && n.y.is_finite(),
            "Configurazione nodo troppo grande o non valida"
        );
        if n.kind == "Comment" {
            ensure!(
                n.text.len() <= 64000
                    && n.width.is_finite()
                    && n.height.is_finite()
                    && (n.width == 0.0 || (220.0..=1200.0).contains(&n.width))
                    && (n.height == 0.0 || (160.0..=1200.0).contains(&n.height)),
                "Commento troppo grande o dimensioni non valide"
            );
        }
        if n.kind == "Agent" {
            ensure!(
                n.runtime.is_empty() || ["codex", "claude", "pi"].contains(&n.runtime.as_str()),
                "Runtime non supportato"
            );
        }
        if ["Task", "Gate"].contains(&n.kind.as_str()) || n.kind == "Review" && n.mode != "human" {
            ensure!(
                w.node(&n.agent)?.kind == "Agent",
                "Il nodo {} richiede un Agent",
                n.title
            );
        }
        if n.kind == "A2A" {
            ensure!(!n.a2a_id.is_empty(), "Seleziona una connessione A2A");
        }
        if n.kind == "RAG" {
            ensure!(
                !n.rag_index.is_empty()
                    && (n.rag_mode.is_empty()
                        || ["keyword", "semantic", "hybrid"].contains(&n.rag_mode.as_str()))
                    && n.rag_top_k <= 30,
                "Seleziona un indice RAG e al massimo 30 risultati"
            );
        }
        if n.kind == "MCP" {
            ensure!(
                !n.mcp_id.is_empty()
                    && n.mcp_tools.len() <= 500
                    && n.mcp_tools.iter().all(|t| t.len() <= 256),
                "Seleziona un server MCP personale e al massimo 500 tool"
            );
        }
        if n.kind == "Workflow" {
            ensure!(
                !n.workflow_id.is_empty() && n.workflow_id != w.id,
                "Seleziona un workflow diverso dal chiamante"
            );
        }
        if n.kind == "Task" {
            ensure!(!n.prompt.trim().is_empty(), "Task senza prompt");
        }
        if n.kind == "Gate" {
            ensure!(
                !n.outcomes.is_empty() && n.outcomes.len() < 12,
                "Gate senza esiti"
            );
            for o in &n.outcomes {
                ensure!(
                    o.len() < 60 && o.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                    "Esito gate non valido"
                );
                if o != "needs_info" && o != "blocked" {
                    ensure!(
                        w.next(&n.id, o).is_some(),
                        "Gate {} senza ramo {}",
                        n.title,
                        o
                    );
                }
            }
        }
        if n.kind == "Memory" {
            ensure!(
                w.node(&n.agent)?.kind == "Task" && w.node(&n.target)?.kind == "Agent",
                "Memory richiede Task sorgente e Agent destinatario"
            );
        }
    }
    ensure!(
        !["Agent", "Source", "MCP", "RAG", "Comment"].contains(&w.node(&w.entry)?.kind.as_str()),
        "Ingresso non eseguibile"
    );
    let mut unique = HashSet::new();
    for e in &w.edges {
        let a = w.node(&e.from)?;
        let b = w.node(&e.to)?;
        ensure!(
            a.kind != "Comment" && b.kind != "Comment",
            "I commenti non hanno collegamenti"
        );
        ensure!(
            a.id != b.id && unique.insert((&e.from, &e.to, &e.kind, &e.outcome)),
            "Arco duplicato o self loop"
        );
        let allowed = edge_allowed(a, b, &e.kind, &e.outcome);
        ensure!(
            allowed,
            "Porte incompatibili: {} → {} ({})",
            a.kind,
            b.kind,
            e.kind
        );
    }
    fn cycle(
        w: &Workflow,
        id: &str,
        vis: &mut HashSet<String>,
        done: &mut HashSet<String>,
    ) -> bool {
        if done.contains(id) {
            return false;
        }
        if !vis.insert(id.into()) {
            return true;
        }
        for e in w
            .edges
            .iter()
            .filter(|e| e.from == id && e.kind == "control")
        {
            if cycle(w, &e.to, vis, done) {
                return true;
            }
        }
        vis.remove(id);
        done.insert(id.into());
        false
    }
    ensure!(
        !cycle(w, &w.entry, &mut HashSet::new(), &mut HashSet::new()),
        "Ciclo in avanti: usa un arco rework con limite"
    );
    let mut live = HashSet::new();
    let mut stack = vec![w.entry.clone()];
    while let Some(x) = stack.pop() {
        if !live.insert(x.clone()) {
            continue;
        }
        for e in w
            .edges
            .iter()
            .filter(|e| e.from == x && e.kind == "control")
        {
            stack.push(e.to.clone())
        }
    }
    for n in w.nodes.iter().filter(|n| n.kind != "Comment") {
        if !["Agent", "Source", "MCP", "RAG"].contains(&n.kind.as_str()) {
            ensure!(live.contains(&n.id), "Nodo non raggiungibile: {}", n.title);
        } else {
            ensure!(
                w.edges.iter().any(|e| e.from == n.id || e.to == n.id),
                "Nodo senza collegamenti: {}",
                n.title
            );
        }
    }
    for n in w
        .nodes
        .iter()
        .filter(|n| ["Source", "Agent", "MCP", "RAG"].contains(&n.kind.as_str()))
    {
        let mut pending = vec![n.id.clone()];
        let mut seen = HashSet::new();
        let mut feeds = false;
        while let Some(x) = pending.pop() {
            if !seen.insert(x.clone()) {
                continue;
            }
            if live.contains(&x) {
                feeds = true;
                break;
            }
            // Review and Gate bind their agent by field rather than a context edge.
            if w.node(&x).is_ok_and(|node| node.kind == "Agent") {
                pending.extend(
                    w.nodes
                        .iter()
                        .filter(|node| {
                            node.agent == x
                                && (node.kind == "Gate"
                                    || node.kind == "Review" && node.mode != "human")
                        })
                        .map(|node| node.id.clone()),
                );
            }
            for e in w
                .edges
                .iter()
                .filter(|e| e.from == x && e.kind == "context")
            {
                pending.push(e.to.clone());
            }
        }
        ensure!(feeds, "{} non è collegato al percorso eseguibile", n.title);
    }
    let mut routes = HashSet::new();
    for e in w
        .edges
        .iter()
        .filter(|e| e.kind == "control" || e.kind == "rework")
    {
        ensure!(
            w.node(&e.from)?.kind == "Fork" || routes.insert((&e.from, &e.outcome)),
            "Più rami per lo stesso esito: {} / {}",
            e.from,
            e.outcome
        );
    }
    // Fork/Join perform explicit automatic handoffs. Other binding changes still need Memory.
    for t in w.nodes.iter().filter(|n| n.kind == "Task") {
        let mut q = vec![(t.id.clone(), t.agent.clone())];
        let mut seen = HashSet::new();
        while let Some((x, binding)) = q.pop() {
            if !seen.insert((x.clone(), binding.clone())) {
                continue;
            }
            for e in w
                .edges
                .iter()
                .filter(|e| e.from == x && e.kind == "control")
            {
                let n = w.node(&e.to)?;
                if n.kind == "Task" {
                    ensure!(
                        n.agent == binding,
                        "{} → {}: cambio Agent senza Memory corretto",
                        t.title,
                        n.title
                    );
                } else if n.kind == "Fork" || n.kind == "Join" {
                    // Structured regions are checked separately; do not carry a single
                    // binding across fan-out or across the result barrier.
                    continue;
                } else if n.kind == "Memory" {
                    ensure!(
                        reachable(w, &n.agent, &n.id),
                        "Memory: Task sorgente fuori percorso"
                    );
                    q.push((n.id.clone(), n.target.clone()));
                } else {
                    q.push((n.id.clone(), binding.clone()));
                }
            }
        }
    }
    Ok(())
}
fn reachable(w: &Workflow, a: &str, b: &str) -> bool {
    let mut seen = HashSet::new();
    let mut s = vec![a.to_string()];
    while let Some(x) = s.pop() {
        if x == b {
            return true;
        }
        if !seen.insert(x.clone()) {
            continue;
        }
        for e in w
            .edges
            .iter()
            .filter(|e| e.from == x && e.kind == "control")
        {
            s.push(e.to.clone());
        }
    }
    false
}

pub async fn create(
    app: &App,
    uid: i64,
    mut w: Workflow,
    input: String,
    base_run: Option<String>,
) -> Result<Run> {
    crate::subworkflows::validate_tree(app, uid, &w)?;
    let settings = app.store.settings(uid)?;
    for n in w.nodes.iter_mut().filter(|n| n.kind == "Agent") {
        if n.runtime.is_empty() {
            n.runtime = settings.default_runtime.clone();
        }
    }
    let rid = id();
    let home = agents::user_home(app, uid)?;
    let run_folder = home.join("workspaces").join(&rid);
    let mut workspace = run_folder.clone();
    let mut source_context = BTreeMap::new();
    let sources: Vec<_> = w.nodes.iter().filter(|n| n.kind == "Source").collect();
    ensure!(
        sources.iter().all(|n| !n.id.is_empty()
            && n.id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')),
        "ID Source non valido"
    );
    ensure!(!sources.is_empty(), "Aggiungi almeno un Source");
    // Resolve every direct binding before changing permissions or preparing files.
    for (i, n) in sources.iter().enumerate() {
        if crate::source_access::direct(n) && !(i == 0 && base_run.is_some()) {
            let path = crate::storage::source_folder(app, "sources", &n.location)?;
            crate::source_access::validate_tree(&path)?;
            source_context.insert(
                format!("direct-source:{}", n.id),
                path.to_string_lossy().into_owned(),
            );
            if i == 0 {
                source_context.insert("direct-primary".into(), n.id.clone());
                workspace = path;
            }
        }
    }
    crate::source_access::available(app, &crate::source_access::paths(&source_context), &[])?;
    for (i, n) in sources.iter().enumerate() {
        if let Some(path) = source_context.get(&format!("direct-source:{}", n.id)) {
            agents::own_tree(app, uid, Path::new(path))?;
            continue;
        }
        let dest = if i == 0 {
            workspace.clone()
        } else {
            run_folder.join(".nodo/sources").join(&n.id)
        };
        if i > 0 && source_context.contains_key("direct-primary") {
            source_context.insert(
                format!("source-path:{}", n.id),
                dest.to_string_lossy().into_owned(),
            );
        }
        if i == 0
            && let Some(base) = &base_run
        {
            let old = app.store.run(uid, base)?;
            ensure!(
                old.status == "completed",
                "Il run di partenza deve essere completato"
            );
            let prior:Vec<_>=app.store.decisions(uid,&old.id)?.into_iter().filter(|d|d.decision=="approve").map(|d|serde_json::json!({"id":d.id,"node":d.node,"user":d.user,"created":d.created})).collect();
            source_context.insert("source-run".into(),serde_json::to_string(&serde_json::json!({"run_id":old.id,"status":old.status,"historical_approvals":prior,"local_delivery":old.outputs.get("commit"),"note":"Historical provenance, not an approval of the new run. Revalidate its changes."}))?);
            agents::copy_tree(Path::new(&old.workspace), &dest)?;
        } else {
            match n.source_type.as_str() {
                "example" | "" => {
                    agents::copy_tree(&app.assets.join("examples/helpdesk"), &dest)?;
                    if n.location == "baseline" {
                        agents::copy_tree(&app.assets.join("examples/living-spec"), &dest)?;
                    }
                }
                "folder" | "examples" => {
                    let namespace = if n.source_type == "examples" {
                        "examples"
                    } else {
                        "sources"
                    };
                    let src = crate::storage::source_folder(app, namespace, &n.location)?;
                    if namespace == "sources" {
                        agents::copy_source_tree(&src, &dest, settings.source_copy_limit_bytes()?)?;
                    } else {
                        agents::copy_tree(&src, &dest)?;
                    }
                }
                "git" => {
                    let url = reqwest::Url::parse(&n.location)?;
                    ensure!(
                        url.scheme() == "https"
                            && url.username().is_empty()
                            && url.password().is_none(),
                        "Repo: usa un URL HTTPS senza credenziali"
                    );
                    std::fs::create_dir_all(&dest)?;
                    agents::own_tree(app, uid, &dest)?;
                    agents::tool(
                        app,
                        uid,
                        &dest,
                        "git",
                        &["clone", "--depth", "1", "--", &n.location, "."],
                        180,
                    )
                    .await?;
                }
                _ => bail!("Tipo Source non supportato"),
            }
        }
    }
    agents::own_tree(app, uid, &workspace)?;
    if run_folder.exists() {
        agents::own_tree(app, uid, &run_folder)?;
    }
    let direct_primary = source_context.contains_key("direct-primary");
    if !direct_primary && !workspace.join(".git").exists() {
        agents::tool(app, uid, &workspace, "git", &["init", "-b", "main"], 30).await?;
        agents::tool(app, uid, &workspace, "git", &["add", "--all"], 30).await?;
        agents::tool(
            app,
            uid,
            &workspace,
            "git",
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-m",
                "Baseline source",
            ],
            30,
        )
        .await?;
    }
    if !direct_primary {
        agents::tool(
            app,
            uid,
            &workspace,
            "git",
            &["checkout", "-b", &format!("nodo/{}", &rid[..8])],
            30,
        )
        .await?;
    }
    let states = w
        .nodes
        .iter()
        .filter(|n| n.kind != "Comment")
        .map(|n| {
            (
                n.id.clone(),
                if ["Source", "Agent", "MCP", "RAG"].contains(&n.kind.as_str()) {
                    "completed"
                } else {
                    "queued"
                }
                .into(),
            )
        })
        .collect();
    let r = Run {
        execution: {
            let mut policy = w.execution.clone();
            if policy.runtime.is_empty() {
                policy.runtime = app.store.settings(uid)?.default_runtime;
            }
            policy
        },
        parent: None,
        children: Vec::new(),
        id: rid,
        workflow: w.clone(),
        status: "paused".into(),
        current: w.entry.clone(),
        created: now(),
        updated: now(),
        workspace: workspace.to_string_lossy().into(),
        states,
        sessions: BTreeMap::new(),
        outputs: BTreeMap::new(),
        pending: None,
        active_edge: w
            .edges
            .iter()
            .find(|e| e.to == w.entry && e.kind == "context")
            .cloned(),
        pause_requested: false,
        attempts: BTreeMap::new(),
        reworks: BTreeMap::new(),
        context: source_context,
        last_task: None,
        last_node: None,
        error: None,
        input,
    };
    crate::artifacts::baseline(app, uid, &r)?;
    app.store.save_run(uid, &r)?;
    emit(
        app,
        uid,
        &r.id,
        "created",
        "",
        "Source preparate secondo la modalità selezionata. Nessun risultato simulato.",
    );
    Ok(r)
}
#[derive(Clone)]
pub struct RunControl {
    pub stop: tokio::sync::watch::Sender<bool>,
    pub done: tokio::sync::watch::Receiver<bool>,
}
pub fn launch(app: App, uid: i64, rid: String) -> Result<()> {
    crate::auth::ensure_active(&app, uid)?;
    let mut queued = app.store.run(uid, &rid)?;
    {
        let mut busy = app.busy.lock().unwrap();
        crate::subworkflows::can_launch_reserved(&app, uid, &queued, &busy)?;
        let lineage = crate::subworkflows::ancestors(&app, uid, &queued)?;
        let root = lineage.last().unwrap_or(&queued);
        for path in crate::source_access::paths(&root.context) {
            crate::source_access::validate_tree(&path)?;
            agents::own_tree(&app, uid, &path)?;
        }
        crate::artifacts::baseline(&app, uid, &queued)?;
        ensure!(busy.insert(rid.clone()), "Esecuzione già attiva");
    }
    queued.status = "starting".into();
    queued.pause_requested = false;
    if let Err(e) = save(&app, uid, &mut queued) {
        app.busy.lock().unwrap().remove(&rid);
        return Err(e);
    }
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let (finished, done) = tokio::sync::watch::channel(false);
    app.workers
        .lock()
        .unwrap()
        .insert(rid.clone(), RunControl { stop, done });
    tokio::spawn(async move {
        // Dropping the drive future drops all active CLI process-group guards,
        // including parallel branches, and releases the scheduler permit.
        let result = tokio::select! {
            biased;
            _ = stopped.changed() => None,
            result = async {
                let _permit = app.slots.acquire(&app.store, uid).await;
                crate::auth::ensure_active(&app, uid)?;
                drive_supervised(&app, uid, &rid).await
            } => Some(result),
        };
        let cancelled = result.is_none();
        if (cancelled || result.as_ref().is_some_and(|r| r.is_err()))
            && let Ok(run) = app.store.run(uid, &rid)
            && let Err(error) = crate::artifacts::snapshot(&app, uid, &run, &run.current, "", "")
        {
            emit(
                &app,
                uid,
                &rid,
                "error",
                &run.current,
                &format!("Raccolta artefatti: {error}"),
            );
        }
        if let Some(Err(e)) = result
            && let Ok(mut r) = app.store.run(uid, &rid)
        {
            r.status = "failed".into();
            r.error = Some(e.to_string());
            r.states.insert(r.current.clone(), "failed".into());
            let _ = save(&app, uid, &mut r);
            emit(&app, uid, &rid, "error", &r.current, &e.to_string());
        }
        app.busy.lock().unwrap().remove(&rid);
        app.workers.lock().unwrap().remove(&rid);
        let _ = finished.send(true);
        if cancelled {
            return;
        }
        let _guard = app.lifecycle.read().await;
        if let Ok(r) = app.store.run(uid, &rid) {
            if r.status == "completed" {
                app.mcp.release_run(uid, &rid).await;
            }
            let _ = crate::subworkflows::settle(&app, uid, &r);
        }
    });
    Ok(())
}
pub(crate) fn save(app: &App, uid: i64, r: &mut Run) -> Result<()> {
    r.updated = now();
    if r.workflow.node(&r.current).is_ok_and(|n| n.kind == "Fork") {
        let live = app.store.run(uid, &r.id)?;
        r.sessions.extend(live.sessions);
        r.pause_requested |= live.pause_requested;
    }
    app.store.save_run(uid, r)?;
    emit(app, uid, &r.id, "state", &r.current, &r.status);
    crate::subworkflows::notify_parent(app, uid, r)?;
    Ok(())
}
fn runtime(r: &Run, agent: &str) -> Result<(String, String)> {
    let a = r.workflow.node(agent)?;
    ensure!(a.kind == "Agent", "Associazione Agent non valida");
    Ok((a.runtime.clone(), a.model.clone()))
}
fn incoming(app: &App, uid: i64, r: &Run) -> Result<String> {
    let sources: Vec<_> = r.workflow.nodes.iter().filter(|n| n.kind == "Source").enumerate().map(|(i,n)| {
        let agents: Vec<_> = r.workflow.edges.iter().filter(|e|e.from==n.id && e.kind=="context").map(|e|e.to.as_str()).collect();
        serde_json::json!({"id":n.id,"title":n.title,"workspace_path":crate::source_access::source_path(r, i, n),"attached_agents":agents})
    }).collect();
    let output = r
        .last_node
        .as_ref()
        .and_then(|id| r.outputs.get(id))
        .map(serde_json::to_string)
        .transpose()?
        .unwrap_or_default();
    let approvals: Vec<_> = app
        .store
        .decisions(uid, &r.id)?
        .into_iter()
        .filter(|d| {
            d.decision == "approve" && r.states.get(&d.node).is_some_and(|s| s == "completed")
        })
        .collect();
    let human_steps: Vec<_> = r
        .workflow
        .nodes
        .iter()
        .filter(|n| n.kind == "Review" && n.mode != "agent")
        .map(|n| serde_json::json!({"id":n.id,"status":r.states.get(&n.id)}))
        .collect();
    Ok(format!(
        "Source disponibili (percorsi relativi al workspace oppure assoluti per le sorgenti dirette; associazioni Agent): {}\nRichiesta originale: {}\nChiarimento autenticato successivo (prevale sulle ambiguità originali): {}\nProvenienza Source: {}\nApprovazioni effettivamente registrate dal server: {}\nReview di approvazione pianificate e stati: {}\nIl server gestisce le richieste di approvazione secondo la policy del run: non attestare esiti futuri, ma non richiedere come prerequisito tecnico una fase umana esplicitamente pianificata a valle.\nOutput precedente: {}\nRisposta autenticata per questo nodo: {}",
        r.context
            .get("inherited-sources")
            .cloned()
            .unwrap_or(serde_json::to_string(&sources)?),
        r.input,
        r.context.get("user-note").cloned().unwrap_or_default(),
        r.context.get("source-run").cloned().unwrap_or_default(),
        serde_json::to_string(&approvals)?,
        serde_json::to_string(&human_steps)?,
        format_args!(
            "{output}\nExecution policy: {}",
            serde_json::to_string(&r.execution)?
        ),
        r.context
            .get(&format!("answer:{}", r.current))
            .cloned()
            .unwrap_or_default()
    ))
}
fn pause_for(
    app: &App,
    uid: i64,
    r: &mut Run,
    kind: &str,
    questions: Vec<String>,
    resume_task: Option<String>,
) -> Result<()> {
    r.status = if kind == "review" || kind == "effect" {
        "waiting_review"
    } else {
        "waiting_user"
    }
    .into();
    r.states.insert(r.current.clone(), r.status.clone());
    r.pending = Some(Pending {
        id: id(),
        proposal: None,
        kind: kind.into(),
        node: r.current.clone(),
        questions,
        resume_task,
    });
    save(app, uid, r)
}
pub(crate) fn route(app: &App, uid: i64, r: &mut Run, outcome: &str) -> Result<()> {
    route_state(r, outcome)?;
    save(app, uid, r)
}
fn route_state(r: &mut Run, outcome: &str) -> Result<()> {
    let edge = r.workflow.next(&r.current, outcome).cloned();
    if let Some(e) = edge {
        if e.kind == "rework" {
            let count = r.reworks.entry(e.from.clone()).or_default();
            *count += 1;
            let max = r.workflow.node(&e.from)?.max_rework.clamp(1, 5);
            ensure!(*count <= max, "Limite di rework ({max}) raggiunto");
            let feedback = r
                .context
                .remove("review-feedback")
                .or_else(|| {
                    r.outputs
                        .get(&e.from)
                        .and_then(|o| serde_json::to_string(o).ok())
                })
                .unwrap_or_else(|| "Review rifiutata: riesamina gli artefatti correnti.".into());
            r.context.insert(format!("feedback:{}", e.to), feedback);
            // Invalidate every reachable downstream result/approval but keep sessions and artifact versions.
            let mut seen = HashSet::new();
            let mut q = vec![e.to.clone()];
            while let Some(x) = q.pop() {
                if !seen.insert(x.clone()) {
                    continue;
                }
                r.states.insert(x.clone(), "queued".into());
                r.outputs.remove(&x);
                r.context.remove(&format!("human-approved:{x}"));
                r.context.remove(&format!("answer:{x}"));
                for e in r
                    .workflow
                    .edges
                    .iter()
                    .filter(|e| e.from == x && e.kind == "control")
                {
                    q.push(e.to.clone());
                }
            }
        }
        r.last_node = Some(r.current.clone());
        r.current = e.to.clone();
        r.active_edge = Some(e);
        r.status = "running".into();
    } else {
        ensure!(
            outcome.is_empty() || outcome == "pass",
            "Nessun ramo per esito {outcome}"
        );
        r.status = "completed".into();
        r.active_edge = None;
        for s in r.states.values_mut() {
            if s == "queued" {
                *s = "skipped".into();
            }
        }
    }
    Ok(())
}
pub(crate) fn tests_current(r: &Run) -> Result<bool> {
    let proofs: Vec<_> = r
        .workflow
        .nodes
        .iter()
        .filter(|n| n.kind == "Connection" && n.provider == "local" && n.operation == "test")
        .filter(|n| {
            r.states.get(&n.id).is_some_and(|s| s == "completed")
                && r.outputs
                    .get(&n.id)
                    .is_some_and(|o| o.status == "completed" && o.decision == "pass")
        })
        .filter_map(|n| r.context.get(&format!("test-proof:{}", n.id)))
        .collect();
    if proofs.is_empty() {
        return Ok(false);
    }
    let current = agents::workspace_fingerprint(Path::new(&r.workspace))?;
    Ok(proofs.iter().any(|proof| **proof == current))
}

async fn drive_supervised(app: &App, uid: i64, rid: &str) -> Result<()> {
    loop {
        if app.store.run(uid, rid)?.pending.is_some()
            && !crate::supervision::resolve(app, uid, rid).await?
        {
            return Ok(());
        }
        drive(app, uid, rid).await?;
        if app.store.run(uid, rid)?.pending.is_none() {
            return Ok(());
        }
    }
}

async fn drive(app: &App, uid: i64, rid: &str) -> Result<()> {
    let mut r = app.store.run(uid, rid)?;
    if r.pause_requested {
        r.status = "paused".into();
        r.pause_requested = false;
        save(app, uid, &mut r)?;
        return Ok(());
    }
    ensure!(
        r.pending.is_none(),
        "Rispondi alla review prima di continuare"
    );
    r.status = "running".into();
    r.error = None;
    r.pause_requested = false;
    save(app, uid, &mut r)?;
    if crate::subworkflows::resume_child(app, uid, &mut r)? {
        return Ok(());
    }
    for _ in 0..500 {
        let live = app.store.run(uid, rid)?;
        if live.pause_requested {
            r.status = "paused".into();
            r.pause_requested = false;
            save(app, uid, &mut r)?;
            return Ok(());
        }
        if r.status != "running" {
            return Ok(());
        }
        let n = r.workflow.node(&r.current)?.clone();
        if r.active_edge.is_none() {
            r.active_edge = r
                .workflow
                .edges
                .iter()
                .find(|e| e.to == n.id && r.last_node.as_ref().is_some_and(|p| p == &e.from))
                .cloned();
        }
        if n.kind == "Fork" {
            execute_parallel(app, uid, &mut r, &n).await?;
            continue;
        }
        if n.kind == "Connection" && connectors::is_write(&n) {
            let digest = connectors::effect_digest(&n, &r, &app.store.settings(uid)?);
            if r.context.remove(&format!("effect:{}", n.id)).as_deref() != Some(&digest) {
                r.context.insert(format!("effect-preview:{}", n.id), digest);
                return pause_for(
                    app,
                    uid,
                    &mut r,
                    "effect",
                    vec![format!(
                        "Autorizzare {} su {}? {} Controlla target e artefatti nelle impostazioni e nel run.",
                        n.operation,
                        n.provider,
                        if n.provider == "atlassian" {
                            format!("twg {}", n.command)
                        } else {
                            String::new()
                        }
                    )],
                    None,
                );
            }
        }
        if n.kind == "Workflow" {
            crate::subworkflows::invoke(app, uid, &mut r, &n)?;
            return Ok(());
        }
        r.states.insert(
            n.id.clone(),
            if ["Connection", "A2A"].contains(&n.kind.as_str()) {
                "waiting_external"
            } else {
                "running"
            }
            .into(),
        );
        *r.attempts.entry(n.id.clone()).or_default() += 1;
        save(app, uid, &mut r)?;
        let mut outcome = String::new();
        let result = match n.kind.as_str() {
            "Join" => {
                let output = r
                    .outputs
                    .get(&n.id)
                    .cloned()
                    .context("Join raggiunto senza risultati paralleli")?;
                join_handoffs(app, uid, &mut r, &n)?;
                output
            }
            "Task" => {
                let (rt, model) = runtime(&r, &n.agent)?;
                let cmd = agents::resolve_command(&n.command, &rt, Path::new(&r.workspace))?;
                let scope = "Strumenti completi: puoi leggere, scrivere, modificare file ed eseguire comandi necessari nel workspace. Rispetta lo scopo della fase SDD: prepara e correggi i documenti prima dell'implementazione, senza anticipare task futuri non richiesti.";
                let handoff = r
                    .context
                    .remove(&format!("handoff:{}", n.agent))
                    .unwrap_or_default();
                let answer = r
                    .context
                    .remove(&format!("answer:{}", n.id))
                    .unwrap_or_default();
                let feedback = r
                    .context
                    .remove(&format!("feedback:{}", n.id))
                    .unwrap_or_default();
                let prompt = format!(
                    "{cmd}\n{scope}\nBinding Agent: {}\n{}\n{}\nHandoff esplicito: {handoff}\nRisposte registrate: {answer}\nFinding da correggere in questo rework: {feedback}",
                    n.agent,
                    n.prompt,
                    incoming(app, uid, &r)?
                );
                let out = agents::turn(
                    app,
                    uid,
                    &mut r,
                    agents::Turn {
                        binding: &n.agent,
                        runtime: &rt,
                        model_override: &model,
                        prompt: &prompt,
                    },
                )
                .await?;
                agents::snapshot(app, uid, &r, &n.id, &n.agent, &out.report())?;
                r.last_task = Some(n.id.clone());
                out
            }
            "Review" => {
                if let Some(receipt) = r.context.remove(&format!("human-approved:{}", n.id)) {
                    let mut out = r.outputs.get(&n.id).cloned().unwrap_or(AgentResult {
                        status: "completed".into(),
                        summary: "Review approvata".into(),
                        ..Default::default()
                    });
                    out.summary
                        .push_str(&format!("\nApprovazione registrata dal server: {receipt}"));
                    out
                } else {
                    if n.mode != "human" {
                        let (rt, model) = runtime(&r, &n.agent)?;
                        let prompt = format!(
                            "REVIEW. {}\n{}\nPuoi modificare i file ed eseguire i comandi necessari per verificare e correggere il lavoro. Riporta le modifiche e i test effettivi. Restituisci decision pass oppure rework.",
                            n.prompt,
                            incoming(app, uid, &r)?
                        );
                        let out = agents::turn(
                            app,
                            uid,
                            &mut r,
                            agents::Turn {
                                binding: &format!("review:{}", n.id),
                                runtime: &rt,
                                model_override: &model,
                                prompt: &prompt,
                            },
                        )
                        .await?;
                        agents::snapshot(
                            app,
                            uid,
                            &r,
                            &n.id,
                            &format!("review:{}", n.id),
                            &out.report(),
                        )?;
                        if out.status == "needs_input" {
                            r.outputs.insert(n.id.clone(), out.clone());
                            let last = r.last_task.clone();
                            return pause_for(app, uid, &mut r, "question", out.questions, last);
                        }
                        ensure!(
                            out.status == "completed",
                            "Review bloccata: {}",
                            out.summary
                        );
                        if out.decision == "rework" {
                            r.outputs.insert(n.id.clone(), out.clone());
                            rework_review(app, uid, &mut r)?;
                            continue;
                        }
                        ensure!(out.decision == "pass", "Review senza decision pass/rework");
                        r.outputs.insert(n.id.clone(), out);
                    }
                    if n.mode != "agent" {
                        return pause_for(app, uid, &mut r, "review", vec![n.prompt.clone()], None);
                    }
                    r.outputs.get(&n.id).cloned().unwrap()
                }
            }
            "Gate" => {
                if n.require_tasks_closed {
                    let open = open_tasks(Path::new(&r.workspace), &n.location)?;
                    let reported_rework = r
                        .last_node
                        .as_ref()
                        .and_then(|id| r.outputs.get(id))
                        .is_some_and(|o| o.decision == "rework");
                    if open > 0 || reported_rework {
                        ensure!(
                            n.outcomes.iter().any(|s| s == "rework"),
                            "Gate di convergenza senza esito rework"
                        );
                        let summary = format!(
                            "Convergenza non raggiunta: {open} task aperti; rapporto precedente rework={reported_rework}. Check deterministico, nessuna approvazione simulata."
                        );
                        r.outputs.insert(
                            n.id.clone(),
                            AgentResult {
                                status: "completed".into(),
                                summary: summary.clone(),
                                decision: "rework".into(),
                                ..Default::default()
                            },
                        );
                        agents::snapshot(app, uid, &r, &n.id, "", &summary)?;
                        r.states.insert(n.id.clone(), "completed".into());
                        route(app, uid, &mut r, "rework")?;
                        continue;
                    }
                }
                if n.require_tests && !tests_current(&r)? {
                    ensure!(
                        n.outcomes.iter().any(|o| o == "rework"),
                        "Quality gate: riesegui i test deterministici sul workspace corrente"
                    );
                    let summary="I test del server sono mancanti o non più validi per i file correnti. Conserva le correzioni e ripassa da implementazione, CI e review prima della consegna.".to_string();
                    agents::snapshot(app, uid, &r, &n.id, "", &summary)?;
                    r.outputs.insert(
                        n.id.clone(),
                        AgentResult {
                            status: "completed".into(),
                            decision: "rework".into(),
                            summary,
                            ..Default::default()
                        },
                    );
                    r.states.insert(n.id.clone(), "completed".into());
                    route(app, uid, &mut r, "rework")?;
                    continue;
                }
                let (rt, model) = runtime(&r, &n.agent)?;
                let prompt = format!(
                    "GATE decisionale con strumenti completi. {}\n{}\nEsiti ammessi: {}. Usa esclusivamente un esito nel campo decision. Puoi modificare file ed eseguire comandi necessari; segnala ogni modifica e rivalida il risultato. Se manca una decisione umana usa status needs_input.",
                    n.prompt,
                    incoming(app, uid, &r)?,
                    n.outcomes.join(", ")
                );
                let mut out = agents::turn(
                    app,
                    uid,
                    &mut r,
                    agents::Turn {
                        binding: &format!("gate:{}", n.id),
                        runtime: &rt,
                        model_override: &model,
                        prompt: &prompt,
                    },
                )
                .await?;
                if out.status == "completed" {
                    if n.require_tests && out.decision == "pass" && !tests_current(&r)? {
                        ensure!(
                            n.outcomes.iter().any(|o| o == "rework"),
                            "Il gate ha cambiato i file dopo CI: nuova verifica richiesta"
                        );
                        out.decision = "rework".into();
                        out.summary = format!(
                            "Il gate ha modificato il workspace dopo i test. Nuova CI e review richieste. {}",
                            out.summary
                        );
                    }
                    ensure!(
                        n.outcomes.contains(&out.decision),
                        "Esito Gate non ammesso: {}",
                        out.decision
                    );
                    outcome = out.decision.clone();
                }
                agents::snapshot(
                    app,
                    uid,
                    &r,
                    &n.id,
                    &format!("gate:{}", n.id),
                    &out.report(),
                )?;
                out
            }
            "Memory" => {
                ensure!(
                    r.outputs.contains_key(&n.agent),
                    "Memory: output Task sorgente assente"
                );
                let text = handoff_package(app, uid, &r, &n.agent, &n.target)?;
                r.context
                    .insert(format!("handoff:{}", n.target), text.clone());
                agents::snapshot(app, uid, &r, &n.id, "", &text)?;
                AgentResult {
                    status: "completed".into(),
                    summary: text,
                    ..Default::default()
                }
            }
            "A2A" => {
                let out = crate::a2a::execute(app, uid, &mut r, &n).await?;
                if out.status == "completed" {
                    agents::snapshot(app, uid, &r, &n.id, "", &out.report())?;
                }
                out
            }
            "Connection" => {
                let out = connectors::execute(app, uid, &r, &n).await?;
                if n.provider == "local"
                    && n.operation == "test"
                    && out.status == "completed"
                    && out.decision == "pass"
                {
                    r.context.insert(
                        format!("test-proof:{}", n.id),
                        agents::workspace_fingerprint(Path::new(&r.workspace))?,
                    );
                }
                agents::snapshot(app, uid, &r, &n.id, "", &out.report())?;
                out
            }
            _ => bail!("Nodo non eseguibile: {}", n.kind),
        };
        if n.kind == "A2A" && result.status == "paused" {
            r.status = "paused".into();
            r.pause_requested = false;
            r.states.insert(n.id.clone(), "paused".into());
            save(app, uid, &mut r)?;
            return Ok(());
        }
        r.outputs.insert(n.id.clone(), result.clone());
        if result.status == "needs_input" {
            let task = Some(n.id.clone());
            return pause_for(app, uid, &mut r, "question", result.questions, task);
        }
        ensure!(
            result.status == "completed",
            "Agente bloccato: {}",
            result.summary
        );
        if n.kind == "Gate" && (outcome == "needs_info" || outcome == "blocked") {
            return pause_for(app, uid, &mut r, "gate", vec![result.summary], None);
        }
        r.states.insert(n.id.clone(), "completed".into());
        route(app, uid, &mut r, &outcome)?;
    }
    bail!("Limite di transizioni del run superato")
}
fn handoff_package(app: &App, uid: i64, r: &Run, source: &str, target: &str) -> Result<String> {
    let output = r
        .outputs
        .get(source)
        .context("Handoff: output sorgente assente")?;
    let arts = app.store.artifacts(uid, &r.id)?;
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "schema":1, "source_task":source, "source_output":output,
        "target_agent":target, "request":r.input,
        "decisions":app.store.decisions(uid,&r.id)?,
        "artifacts":arts.iter().map(|a|serde_json::json!({"id":a.id,"path":a.path,"version":a.version,"sha256":a.hash})).collect::<Vec<_>>(),
        "limits":"Context handoff; native model-private state is not copied."
    }))?)
}

// Deliver the barrier package even when Review/Gate steps intervene before the
// consumer. Memory and subsequent parallel regions establish their own handoffs.
fn join_handoffs(app: &App, uid: i64, r: &mut Run, join: &Node) -> Result<()> {
    let mut queue = vec![join.id.clone()];
    let mut seen = HashSet::new();
    while let Some(id) = queue.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        for edge in r
            .workflow
            .edges
            .iter()
            .filter(|e| e.from == id && e.kind == "control")
        {
            let node = r.workflow.node(&edge.to)?;
            match node.kind.as_str() {
                "Task" => {
                    let package = handoff_package(app, uid, r, &join.id, &node.agent)?;
                    r.context.insert(format!("handoff:{}", node.agent), package);
                }
                "Memory" | "Fork" | "Join" => {}
                _ => queue.push(node.id.clone()),
            }
        }
    }
    Ok(())
}

fn parallel_prompt(app: &App, uid: i64, branch: &Run, task: &Node, fork: &Node) -> Result<String> {
    let (rt, _) = runtime(branch, &task.agent)?;
    let cmd = agents::resolve_command(&task.command, &rt, Path::new(&branch.workspace))?;
    let handoff = if let Some(source) = branch
        .last_node
        .as_ref()
        .filter(|id| branch.outputs.contains_key(*id))
    {
        handoff_package(app, uid, branch, source, &task.agent)?
    } else {
        branch
            .context
            .get(&format!("handoff:{}", task.agent))
            .cloned()
            .unwrap_or_default()
    };
    Ok(format!(
        "{}\n{}\n{}\nHandoff esplicito: {}\nChiarimento per i rami: {}\nTask parallelo: workspace condiviso, rispetta i file assegnati al tuo ramo.",
        cmd,
        task.prompt,
        incoming(app, uid, branch)?,
        handoff,
        branch
            .context
            .get(&format!("answer:{}", fork.id))
            .cloned()
            .unwrap_or_default()
    ))
}

async fn execute_parallel(app: &App, uid: i64, r: &mut Run, fork: &Node) -> Result<()> {
    use futures::{StreamExt, stream::FuturesUnordered};
    let branches = parallel_branches(&r.workflow, fork)?;
    r.states.insert(fork.id.clone(), "running".into());
    loop {
        if app.store.run(uid, &r.id)?.pause_requested {
            r.status = "paused".into();
            r.pause_requested = false;
            return save(app, uid, r);
        }
        let ready: Vec<_> = branches
            .iter()
            .filter_map(|branch| {
                branch
                    .iter()
                    .find(|n| r.states.get(&n.id).is_none_or(|s| s != "completed"))
                    .cloned()
            })
            .collect();
        if ready.is_empty() {
            break;
        }
        for n in &ready {
            r.states.insert(n.id.clone(), "running".into());
            *r.attempts.entry(n.id.clone()).or_default() += 1;
        }
        save(app, uid, r)?;
        let mut work = FuturesUnordered::new();
        for n in ready {
            let mut branch = r.clone();
            branch.current = n.id.clone();
            branch.context.insert("parallel-turn".into(), "true".into());
            if let Some(chain) = branches.iter().find(|b| b.iter().any(|t| t.id == n.id))
                && let Some(previous) = chain.iter().take_while(|t| t.id != n.id).last()
            {
                branch.last_node = Some(previous.id.clone());
            }
            work.push(async move {
                let result: Result<AgentResult> = async {
                    let (rt, model) = runtime(&branch, &n.agent)?;
                    let prompt = parallel_prompt(app, uid, &branch, &n, fork)?;
                    agents::turn(
                        app,
                        uid,
                        &mut branch,
                        agents::Turn {
                            binding: &n.agent,
                            runtime: &rt,
                            model_override: &model,
                            prompt: &prompt,
                        },
                    )
                    .await
                }
                .await;
                (n, branch, result)
            });
        }
        let mut questions = Vec::new();
        let mut errors = Vec::new();
        while let Some((n, branch, result)) = work.next().await {
            if let Some(session) = branch.sessions.get(&n.agent) {
                r.sessions.insert(n.agent.clone(), session.clone());
            }
            match result {
                Ok(out) => {
                    if let Err(e) = agents::snapshot(app, uid, r, &n.id, &n.agent, &out.report()) {
                        errors.push(format!("{}: {e}", n.title));
                    }
                    let state = if out.status == "completed" {
                        "completed"
                    } else if out.status == "needs_input" {
                        "waiting_user"
                    } else {
                        "failed"
                    };
                    r.states.insert(n.id.clone(), state.into());
                    if state == "waiting_user" {
                        questions
                            .extend(out.questions.iter().map(|q| format!("{}: {}", n.title, q)));
                    }
                    if state == "failed" {
                        errors.push(format!("{}: {}", n.title, out.summary));
                    }
                    r.outputs.insert(n.id.clone(), out);
                }
                Err(e) => {
                    r.states.insert(n.id.clone(), "failed".into());
                    errors.push(format!("{}: {e}", n.title));
                }
            }
            save(app, uid, r)?;
        }
        ensure!(
            errors.is_empty(),
            "Rami paralleli falliti: {}",
            errors.join("; ")
        );
        if !questions.is_empty() {
            return pause_for(app, uid, r, "question", questions, Some(fork.id.clone()));
        }
    }
    let outputs: BTreeMap<_, _> = branches
        .iter()
        .flatten()
        .map(|n| {
            let output = r
                .outputs
                .get(&n.id)
                .context("Join: risultato di un ramo assente")?;
            ensure!(
                output.status == "completed",
                "Join: risultato di un ramo incompleto"
            );
            Ok((n.id.clone(), output))
        })
        .collect::<Result<_>>()?;
    r.outputs.insert(
        fork.target.clone(),
        AgentResult {
            status: "completed".into(),
            summary: serde_json::to_string(&outputs)?,
            ..Default::default()
        },
    );
    r.states.insert(fork.id.clone(), "completed".into());
    r.last_node = Some(fork.id.clone());
    r.current = fork.target.clone();
    r.active_edge = None;
    save(app, uid, r)
}

fn rework_review(app: &App, uid: i64, r: &mut Run) -> Result<()> {
    rework_review_state(r)?;
    save(app, uid, r)
}
fn rework_review_state(r: &mut Run) -> Result<()> {
    if !r.context.contains_key("review-feedback")
        && let Some(out) = r.outputs.get(&r.current)
    {
        r.context
            .insert("review-feedback".into(), serde_json::to_string(out)?);
    }
    if r.workflow.next(&r.current, "rework").is_some() {
        return route_state(r, "rework");
    }
    let gate = r
        .workflow
        .next(&r.current, "")
        .and_then(|e| r.workflow.node(&e.to).ok())
        .filter(|n| n.kind == "Gate")
        .context("Review rifiutata senza ramo rework configurato")?
        .id
        .clone();
    r.states.insert(r.current.clone(), "rework".into());
    r.current = gate;
    route_state(r, "rework")
}
pub fn decide(app: &App, user: &User, rid: &str, action: &str, answer: &str) -> Result<Run> {
    ensure!(
        !app.busy.lock().unwrap().contains(rid),
        "Attendi la chiusura del turno attivo"
    );
    apply_decision(app, user, rid, action, answer, None)
}
pub(crate) fn apply_decision(
    app: &App,
    user: &User,
    rid: &str,
    action: &str,
    answer: &str,
    supervisor: Option<SupervisorProposal>,
) -> Result<Run> {
    let mut r = app.store.run(user.id, rid)?;
    let p = r.pending.clone().context("Nessuna decisione in attesa")?;
    ensure!(
        supervisor.is_none() || r.execution.mode == ExecutionMode::Unattended,
        "Decisione automatica non autorizzata"
    );
    ensure!(
        supervisor.is_none()
            || p.kind != "effect"
            || action != "approve"
            || r.execution.allow_external_writes,
        "Scritture esterne non autorizzate"
    );
    ensure!(
        ["approve", "reject", "answer"].contains(&action),
        "Azione sconosciuta"
    );
    if p.kind == "question" || p.kind == "gate" {
        ensure!(
            action == "answer" && !answer.trim().is_empty(),
            "Risposta obbligatoria"
        );
    } else {
        ensure!(action != "answer", "Approva o rifiuta questa review");
    }
    let artifacts = app
        .store
        .artifacts(user.id, rid)?
        .iter()
        .map(|a| a.id.clone())
        .collect();
    let receipt = Decision {
        actor: if supervisor.is_some() {
            "ai"
        } else if user.email.starts_with("a2a:") {
            "a2a"
        } else {
            "human"
        }
        .into(),
        id: if supervisor.is_some() {
            format!("ai:{}", p.id)
        } else {
            id()
        },
        supervisor,
        run: rid.into(),
        node: p.node.clone(),
        decision: action.into(),
        answer: answer.into(),
        user: user.email.clone(),
        created: now(),
        artifacts,
    };
    r.pending = None;
    r.status = "paused".into();
    r.error = None;
    match action {
        "reject" => {
            r.context.insert(
                "review-feedback".into(),
                serde_json::to_string(
                    &serde_json::json!({"approval":receipt,"review_output":r.outputs.get(&p.node)}),
                )?,
            );
            if p.kind == "effect" {
                r.status = "failed".into();
                r.error = Some("Operazione remota rifiutata; nessuna scrittura eseguita".into());
            } else if rework_review_state(&mut r).is_err() {
                r.status = "failed".into();
                r.error = Some(
                    "Review rifiutata. Correggi il workflow o avvia una nuova esecuzione.".into(),
                );
            }
        }
        "approve" => {
            if p.kind == "effect" {
                let digest = r
                    .context
                    .remove(&format!("effect-preview:{}", p.node))
                    .context("Anteprima effetto mancante")?;
                r.context.insert(format!("effect:{}", p.node), digest);
            } else {
                r.context.insert(
                    format!("human-approved:{}", p.node),
                    serde_json::to_string(&receipt)?,
                );
            }
        }
        "answer" => {
            let target = if ["Gate", "Review"].contains(&r.workflow.node(&p.node)?.kind.as_str()) {
                p.node.clone()
            } else {
                p.resume_task.unwrap_or_else(|| p.node.clone())
            };
            r.context
                .insert(format!("answer:{target}"), serde_json::to_string(&receipt)?);
            r.last_node = Some(p.node.clone());
            if target != p.node {
                r.active_edge = None;
            }
            r.current = target.clone();
            r.states.insert(target, "queued".into());
        }
        _ => unreachable!(),
    }
    app.store
        .checkpoint_decision(user.id, &r, &receipt, &p.id)?;
    save(app, user.id, &mut r)?;
    emit(
        app,
        user.id,
        rid,
        if receipt.actor == "ai" {
            "ai_decision"
        } else {
            "decision"
        },
        &p.node,
        &format!("{} · {}", action, answer),
    );
    Ok(r)
}
pub fn annotate(app: &App, user: &User, rid: &str, text: &str) -> Result<Run> {
    ensure!(
        !app.busy.lock().unwrap().contains(rid),
        "Metti il run in pausa prima di aggiungere contesto"
    );
    ensure!(
        !text.trim().is_empty() && text.len() <= 12000,
        "Chiarimento obbligatorio (max 12000 caratteri)"
    );
    let mut r = app.store.run(user.id, rid)?;
    ensure!(
        ["paused", "waiting_review", "waiting_user"].contains(&r.status.as_str()),
        "Il run deve essere in pausa o in attesa di una decisione"
    );
    let receipt = Decision {
        actor: "human".into(),
        supervisor: None,
        id: id(),
        run: rid.into(),
        node: r.current.clone(),
        decision: "clarification".into(),
        answer: text.into(),
        user: user.email.clone(),
        created: now(),
        artifacts: app
            .store
            .artifacts(user.id, rid)?
            .into_iter()
            .map(|a| a.id)
            .collect(),
    };
    app.store.add_decision(user.id, &receipt)?;
    r.context
        .insert("user-note".into(), serde_json::to_string(&receipt)?);
    save(app, user.id, &mut r)?;
    Ok(r)
}
pub(crate) fn open_tasks(workspace: &Path, location: &str) -> Result<usize> {
    let workspace = &std::fs::canonicalize(workspace)?;
    let path = if location.is_empty() {
        let specs = workspace.join("specs");
        let matches: Vec<_> = std::fs::read_dir(specs)?
            .filter_map(|e| e.ok())
            .map(|e| e.path().join("tasks.md"))
            .filter(|p| p.is_file())
            .collect();
        ensure!(
            matches.len() == 1,
            "Seleziona il percorso tasks.md nel Gate: feature assente o ambigua"
        );
        matches[0].clone()
    } else {
        workspace.join(location)
    };
    let path = std::fs::canonicalize(path)?;
    ensure!(path.starts_with(workspace), "tasks.md fuori workspace");
    let text = std::fs::read_to_string(path)?;
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    let is_open =
        |s: &str| s.starts_with("- [ ]") || s.starts_with("* [ ]") || s.starts_with("+ [ ]");
    ensure!(
        lines
            .iter()
            .any(|s| is_open(s) || s.starts_with("- [x]") || s.starts_with("- [X]")),
        "tasks.md senza task verificabili"
    );
    Ok(lines.iter().filter(|s| is_open(s)).count())
}

#[cfg(test)]
mod parallel_tests {
    use super::*;
    fn graph() -> Workflow {
        serde_json::from_value(serde_json::json!({
            "id":"parallel", "name":"Parallel", "description":"", "entry":"fork",
            "nodes":[{"id":"fork","kind":"Fork","target":"join"},
                {"id":"a","kind":"Task","agent":"aa","prompt":"A"},
                {"id":"b","kind":"Task","agent":"ab","prompt":"B"},
                {"id":"aa","kind":"Agent"},{"id":"ab","kind":"Agent"},
                {"id":"join","kind":"Join"},{"id":"source","kind":"Source","source_type":"example"}],
            "edges":[{"from":"source","to":"aa","kind":"context"},
                {"from":"aa","to":"a","kind":"context"},
                {"from":"ab","to":"b","kind":"context"},
                {"from":"fork","to":"a","kind":"control"},
                {"from":"fork","to":"b","kind":"control"},
                {"from":"a","to":"join","kind":"control"},
                {"from":"b","to":"join","kind":"control"}]
        }))
        .unwrap()
    }
    fn coordinator_graph() -> Workflow {
        let mut w = graph();
        w.entry = "plan".into();
        for value in [
            serde_json::json!({"id":"coordinator","kind":"Agent"}),
            serde_json::json!({"id":"plan","kind":"Task","agent":"coordinator","prompt":"Analyze brief"}),
            serde_json::json!({"id":"synthesis","kind":"Task","agent":"coordinator","prompt":"Editorial synthesis"}),
        ] {
            w.nodes.push(serde_json::from_value(value).unwrap());
        }
        for (from, to, kind) in [
            ("coordinator", "plan", "context"),
            ("coordinator", "synthesis", "context"),
            ("plan", "fork", "control"),
            ("join", "synthesis", "control"),
        ] {
            w.edges.push(Edge {
                from: from.into(),
                to: to.into(),
                kind: kind.into(),
                outcome: String::new(),
            });
        }
        w
    }
    fn completed(summary: &str) -> AgentResult {
        AgentResult {
            status: "completed".into(),
            summary: summary.into(),
            ..Default::default()
        }
    }
    #[test]
    fn accepts_coordinator_fanout_and_synthesis_but_rejects_sequential_binding_changes() {
        validate(&coordinator_graph()).unwrap();
        let mut w = coordinator_graph();
        w.nodes.push(serde_json::from_value(serde_json::json!({"id":"a2","kind":"Task","agent":"coordinator","prompt":"Wrong binding"})).unwrap());
        w.edges
            .iter_mut()
            .find(|e| e.from == "a" && e.to == "join")
            .unwrap()
            .to = "a2".into();
        w.edges.push(Edge {
            from: "a2".into(),
            to: "join".into(),
            kind: "control".into(),
            outcome: String::new(),
        });
        assert!(validate(&w).unwrap_err().to_string().contains("Memory"));
        let mut w = coordinator_graph();
        w.nodes
            .iter_mut()
            .find(|n| n.id == "synthesis")
            .unwrap()
            .agent = "aa".into();
        w.edges
            .iter_mut()
            .find(|e| e.to == "synthesis" && e.kind == "context")
            .unwrap()
            .from = "aa".into();
        // A new consumer after the Join is supported as well as the original Coordinator.
        validate(&w).unwrap();
    }
    #[tokio::test]
    async fn coordinator_plan_reaches_both_branches_and_join_hands_all_results_back() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let w = coordinator_graph();
        let mut r = create(&app, 1, w.clone(), "Research request".into(), None)
            .await
            .unwrap();
        r.outputs
            .insert("plan".into(), completed("Coordinator research plan"));
        r.last_node = Some("plan".into());
        r.current = "fork".into();
        let fork = w.node("fork").unwrap();
        for id in ["a", "b"] {
            let task = w.node(id).unwrap();
            let prompt = parallel_prompt(&app, 1, &r, task, fork).unwrap();
            assert!(prompt.contains("Coordinator research plan"));
            assert!(prompt.contains("Research request"));
            assert!(prompt.contains(&task.agent));
            assert!(prompt.contains("native model-private state is not copied"));
        }
        r.states.insert("a".into(), "completed".into());
        r.outputs
            .insert("a".into(), completed("Research A findings"));
        // Resuming the other branch retains the Coordinator's plan, not its sibling's result.
        let prompt = parallel_prompt(&app, 1, &r, w.node("b").unwrap(), fork).unwrap();
        assert!(prompt.contains("Coordinator research plan"));
        assert!(!prompt.contains("Research A findings"));
        r.states.insert("b".into(), "completed".into());
        r.outputs
            .insert("b".into(), completed("Research B findings"));
        execute_parallel(&app, 1, &mut r, fork).await.unwrap();
        join_handoffs(&app, 1, &mut r, w.node("join").unwrap()).unwrap();
        let package: serde_json::Value =
            serde_json::from_str(&r.context["handoff:coordinator"]).unwrap();
        assert_eq!(package["source_task"], "join");
        assert_eq!(package["target_agent"], "coordinator");
        let results: serde_json::Value =
            serde_json::from_str(package["source_output"]["summary"].as_str().unwrap()).unwrap();
        assert_eq!(results["a"]["summary"], "Research A findings");
        assert_eq!(results["b"]["summary"], "Research B findings");
        assert!(r.attempts.is_empty());
        // The handoff is persisted with the same state used for pause/restart.
        save(&app, 1, &mut r).unwrap();
        assert_eq!(
            app.store.run(1, &r.id).unwrap().context["handoff:coordinator"],
            r.context["handoff:coordinator"]
        );
        r.outputs.remove("b");
        assert!(
            execute_parallel(&app, 1, &mut r, fork)
                .await
                .unwrap_err()
                .to_string()
                .contains("assente")
        );
        r.outputs.insert(
            "b".into(),
            AgentResult {
                status: "failed".into(),
                ..Default::default()
            },
        );
        assert!(
            execute_parallel(&app, 1, &mut r, fork)
                .await
                .unwrap_err()
                .to_string()
                .contains("incompleto")
        );
    }
    #[tokio::test]
    async fn drive_persists_join_handoff_before_human_review() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let mut w = coordinator_graph();
        w.nodes.push(
            serde_json::from_value(serde_json::json!({
                "id":"review", "kind":"Review", "mode":"human", "prompt":"Review research"
            }))
            .unwrap(),
        );
        w.edges.iter_mut().find(|e| e.from == "join").unwrap().to = "review".into();
        w.edges.push(Edge {
            from: "review".into(),
            to: "synthesis".into(),
            kind: "control".into(),
            outcome: String::new(),
        });
        let mut r = create(&app, 1, w.clone(), "Research".into(), None)
            .await
            .unwrap();
        r.current = "fork".into();
        r.last_node = Some("plan".into());
        r.outputs.insert("plan".into(), completed("Plan"));
        for id in ["a", "b"] {
            r.states.insert(id.into(), "completed".into());
            r.outputs.insert(id.into(), completed(id));
        }
        save(&app, 1, &mut r).unwrap();
        drive(&app, 1, &r.id).await.unwrap();
        let saved = app.store.run(1, &r.id).unwrap();
        assert_eq!(saved.status, "waiting_review");
        assert_eq!(saved.pending.unwrap().node, "review");
        assert_eq!(saved.states["join"], "completed");
        assert!(!saved.attempts.contains_key("a"));
        assert!(!saved.attempts.contains_key("b"));
        let package: serde_json::Value =
            serde_json::from_str(&saved.context["handoff:coordinator"]).unwrap();
        assert_eq!(package["source_task"], "join");
        assert_eq!(package["target_agent"], "coordinator");
        let results: serde_json::Value =
            serde_json::from_str(package["source_output"]["summary"].as_str().unwrap()).unwrap();
        assert_eq!(results["a"]["summary"], "a");
        assert_eq!(results["b"]["summary"], "b");
    }
    #[test]
    fn accepts_structured_parallel_tasks() {
        validate(&graph()).unwrap();
    }
    #[test]
    fn rejects_shared_sessions_and_unjoined_paths() {
        let mut w = graph();
        w.nodes[2].agent = "aa".into();
        assert!(
            validate_parallel(&w)
                .unwrap_err()
                .to_string()
                .contains("distinti")
        );
        let mut w = graph();
        w.edges.pop();
        assert!(validate(&w).is_err());
        let mut w = graph();
        w.nodes[0].target = "a".into();
        assert!(validate(&w).is_err());
    }
    #[tokio::test]
    async fn barrier_collects_results_without_repeating_completed_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let w = graph();
        let mut r = create(&app, 1, w.clone(), String::new(), None)
            .await
            .unwrap();
        for id in ["a", "b"] {
            r.states.insert(id.into(), "completed".into());
            r.outputs.insert(
                id.into(),
                AgentResult {
                    status: "completed".into(),
                    summary: id.into(),
                    ..Default::default()
                },
            );
        }
        execute_parallel(&app, 1, &mut r, &w.nodes[0])
            .await
            .unwrap();
        assert_eq!(r.current, "join");
        assert!(r.attempts.is_empty());
        assert!(r.outputs["join"].summary.contains("a"));
        assert!(r.outputs["join"].summary.contains("b"));
    }
}
