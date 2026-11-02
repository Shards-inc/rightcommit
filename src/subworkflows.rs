//! Persistent workflow calls. Children share files, never native agent sessions.
use crate::{App, agents, engine, model::*};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

pub const MAX_DEPTH: usize = 5;
pub fn validate_tree(app: &App, uid: i64, w: &Workflow) -> Result<()> {
    fn visit(app: &App, uid: i64, w: &Workflow, stack: &mut Vec<String>) -> Result<()> {
        ensure!(
            stack.len() < MAX_DEPTH,
            "Massimo 5 livelli di workflow annidati"
        );
        ensure!(!stack.contains(&w.id), "Richiamo circolare tra workflow");
        engine::validate(w)?;
        let settings = app.store.settings(uid)?;
        for n in w.nodes.iter().filter(|n| n.kind == "A2A") {
            crate::a2a::validate_connection(app, uid, &n.a2a_id)?;
        }
        for n in w.nodes.iter().filter(|n| n.kind == "MCP") {
            ensure!(
                settings.mcp_servers.contains_key(&n.mcp_id),
                "Server MCP personale non trovato: {}",
                n.title
            );
        }
        for n in w.nodes.iter().filter(|n| n.kind == "RAG") {
            let i = crate::rag::get(app, uid, &n.rag_index)?;
            ensure!(
                i.directory && i.generation.is_some(),
                "Seleziona una cartella RAG indicizzata: {}",
                n.title
            );
        }
        stack.push(w.id.clone());
        for n in w.nodes.iter().filter(|n| n.kind == "Workflow") {
            let child = app
                .store
                .workflow(uid, &n.workflow_id)
                .with_context(|| format!("{}: workflow richiamato non disponibile", n.title))?;
            visit(app, uid, &child, stack)?;
        }
        stack.pop();
        Ok(())
    }
    visit(app, uid, w, &mut Vec::new())
}
pub fn ancestors(app: &App, uid: i64, r: &Run) -> Result<Vec<Run>> {
    let mut out = Vec::new();
    let mut current = r.clone();
    let mut seen = HashSet::from([r.id.clone()]);
    while let Some(p) = &current.parent {
        ensure!(
            out.len() < MAX_DEPTH && seen.insert(p.run.clone()),
            "Gerarchia run non valida"
        );
        let parent = app.store.run(uid, &p.run)?;
        ensure!(
            parent
                .children
                .iter()
                .any(|c| c.run == current.id && c.node == p.node),
            "Collegamento padre non valido"
        );
        out.push(parent.clone());
        current = parent;
    }
    Ok(out)
}
pub fn workspace(app: &App, uid: i64, r: &Run) -> Result<PathBuf> {
    let lineage = ancestors(app, uid, r)?;
    let root = lineage.last().unwrap_or(r);
    crate::source_access::validate_bindings(app, root)?;
    if let Some(id) = root.context.get("direct-primary") {
        let path = root
            .context
            .get(&format!("direct-source:{id}"))
            .context("Source primaria assente")?;
        ensure!(
            std::path::Path::new(path) == std::path::Path::new(&r.workspace),
            "Workspace diretto non valido"
        );
        return Ok(PathBuf::from(path));
    }
    let expected = app.data.join(format!("users/{uid}/workspaces/{}", root.id));
    ensure!(
        uuid::Uuid::parse_str(&root.id).is_ok()
            && !expected
                .ancestors()
                .take_while(|p| p.starts_with(&app.data))
                .any(|p| p.is_symlink()),
        "Workspace non valido"
    );
    ensure!(
        std::fs::canonicalize(&expected)? == std::fs::canonicalize(&r.workspace)?,
        "Workspace non valido"
    );
    Ok(expected)
}
pub fn can_launch(app: &App, uid: i64, r: &Run) -> Result<()> {
    can_launch_reserved(app, uid, r, &app.busy.lock().unwrap())
}
pub fn can_launch_reserved(app: &App, uid: i64, r: &Run, busy: &HashSet<String>) -> Result<()> {
    let lineage = ancestors(app, uid, r)?;
    let mut child = r;
    for p in &lineage {
        let link = child
            .parent
            .as_ref()
            .context("Collegamento padre assente")?;
        ensure!(
            p.current == link.node
                && active_call(p).is_some_and(|c| c.run == child.id)
                && ["waiting_child", "waiting_review", "waiting_user"].contains(&p.status.as_str())
                && !p.pause_requested,
            "Riprendi prima il workflow principale"
        );
        child = p;
    }
    let root = lineage.last().unwrap_or(r);
    crate::source_access::validate_bindings(app, root)?;
    let excluded: Vec<_> = lineage
        .iter()
        .map(|a| a.id.clone())
        .chain([r.id.clone()])
        .collect();
    crate::source_access::available(app, &crate::source_access::paths(&root.context), &excluded)?;
    for id in busy {
        if id == &r.id {
            continue;
        }
        let other = app.store.run(uid, id);
        if let Ok(other) = other {
            ensure!(
                other.workspace != r.workspace
                    || lineage
                        .iter()
                        .any(|a| a.id == *id && a.status == "waiting_child"),
                "Un'esecuzione sta già scrivendo nello stesso workspace"
            );
        }
    }
    Ok(())
}
fn active_call(r: &Run) -> Option<&ChildRun> {
    if r.workflow.node(&r.current).ok()?.kind != "Workflow"
        || r.states
            .get(&r.current)
            .is_none_or(|s| ["queued", "completed", "skipped"].contains(&s.as_str()))
    {
        return None;
    }
    r.children
        .iter()
        .rev()
        .find(|c| c.node == r.current && Some(&c.attempt) == r.attempts.get(&r.current))
}
/// Forward child state through the call node, but keep human decisions on the child.
pub fn notify_parent(app: &App, uid: i64, child: &Run) -> Result<()> {
    let Some(link) = &child.parent else {
        return Ok(());
    };
    let mut parent = app.store.run(uid, &link.run)?;
    if parent.current != link.node || active_call(&parent).is_none_or(|c| c.run != child.id) {
        return Ok(());
    }
    let status = match child.status.as_str() {
        "waiting_review" => "waiting_review",
        "waiting_user" => "waiting_user",
        "failed" | "interrupted" => "failed",
        "paused" if parent.pause_requested => "paused",
        _ => "waiting_child",
    };
    parent.status = if parent.pause_requested && child.status == "completed" {
        "paused"
    } else {
        status
    }
    .into();
    parent.states.insert(
        link.node.clone(),
        if status == "waiting_child" {
            "running"
        } else {
            status
        }
        .into(),
    );
    parent.error = if status == "failed" {
        Some(format!(
            "Workflow figlio {}: {}",
            child.workflow.name,
            child.error.as_deref().unwrap_or("Esecuzione interrotta")
        ))
    } else {
        None
    };
    parent.updated = now();
    app.store.save_run(uid, &parent)?;
    agents::emit(app, uid, &parent.id, "state", &link.node, &parent.status);
    notify_parent(app, uid, &parent)
}
fn wake(app: &App, uid: i64, parent: &Run) -> Result<()> {
    let Some(call) = active_call(parent) else {
        return Ok(());
    };
    let child = app.store.run(uid, &call.run)?;
    if child.status == "completed"
        && !parent.pause_requested
        && !["paused", "interrupted", "failed"].contains(&parent.status.as_str())
        && !app.busy.lock().unwrap().contains(&parent.id)
    {
        engine::launch(app.clone(), uid, parent.id.clone())?;
    }
    Ok(())
}
/// Called after releasing a worker slot, also covering children that finish immediately.
pub fn settle(app: &App, uid: i64, r: &Run) -> Result<()> {
    wake(app, uid, r)?;
    if let Some(p) = &r.parent {
        let parent = app.store.run(uid, &p.run)?;
        wake(app, uid, &parent)?;
    }
    Ok(())
}
pub fn invoke(app: &App, uid: i64, parent: &mut Run, node: &Node) -> Result<()> {
    let mut w = app.store.workflow(uid, &node.workflow_id)?;
    validate_tree(app, uid, &w)?;
    let lineage = ancestors(app, uid, parent)?;
    ensure!(
        lineage.len() + 2 <= MAX_DEPTH,
        "Massimo 5 livelli di workflow annidati"
    );
    let ids: HashSet<_> = lineage
        .iter()
        .map(|p| p.workflow.id.as_str())
        .chain([parent.workflow.id.as_str()])
        .collect();
    fn excludes(app: &App, uid: i64, w: &Workflow, ids: &HashSet<&str>) -> Result<()> {
        ensure!(
            !ids.contains(w.id.as_str()),
            "Richiamo circolare tra workflow"
        );
        for n in w.nodes.iter().filter(|n| n.kind == "Workflow") {
            excludes(app, uid, &app.store.workflow(uid, &n.workflow_id)?, ids)?;
        }
        Ok(())
    }
    excludes(app, uid, &w, &ids)?;
    workspace(app, uid, parent)?;
    let settings = app.store.settings(uid)?;
    for a in w.nodes.iter_mut().filter(|n| n.kind == "Agent") {
        if a.runtime.is_empty() {
            a.runtime = settings.default_runtime.clone();
        }
    }
    let rid = id();
    let arts = app.store.artifacts(uid, &parent.id)?;
    let package = serde_json::json!({"schema":1,"parent_run":parent.id,"call_node":node.id,"request":parent.input,"instructions":node.prompt,"previous_output":parent.last_node.as_ref().and_then(|n|parent.outputs.get(n)),"user_note":parent.context.get("user-note"),"decisions":app.store.decisions(uid,&parent.id)?,"artifacts":arts.iter().map(|a|serde_json::json!({"id":a.id,"path":a.path,"version":a.version,"sha256":a.hash})).collect::<Vec<_>>(),"session_policy":"Separate native sessions. Shared workspace, sequential execution. Historical approvals do not approve child changes."});
    let sources = parent.context.get("inherited-sources").cloned().unwrap_or_else(|| {
        serde_json::to_string(&parent.workflow.nodes.iter().filter(|n| n.kind == "Source").enumerate().map(|(i,n)| serde_json::json!({"id":n.id,"title":n.title,"workspace_path":crate::source_access::source_path(parent, i, n)})).collect::<Vec<_>>()).unwrap()
    });
    let child = Run {
        execution: parent.execution.clone(),
        id: rid.clone(),
        parent: Some(ParentRun {
            run: parent.id.clone(),
            node: node.id.clone(),
        }),
        children: vec![],
        current: w.entry.clone(),
        states: w
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
            .collect(),
        workflow: w,
        status: "paused".into(),
        created: now(),
        updated: now(),
        workspace: parent.workspace.clone(),
        sessions: BTreeMap::new(),
        outputs: BTreeMap::new(),
        pending: None,
        active_edge: None,
        pause_requested: false,
        attempts: BTreeMap::new(),
        reworks: BTreeMap::new(),
        context: BTreeMap::from([("inherited-sources".into(), sources)]),
        last_task: None,
        last_node: None,
        error: None,
        input: serde_json::to_string_pretty(&package)?,
    };
    let attempt = parent.attempts.entry(node.id.clone()).or_default();
    *attempt += 1;
    parent.children.push(ChildRun {
        run: rid.clone(),
        node: node.id.clone(),
        attempt: *attempt,
    });
    parent.status = "waiting_child".into();
    parent.states.insert(node.id.clone(), "running".into());
    parent.updated = now();
    // Persist the link and the child atomically: retry never creates duplicate child runs.
    {
        let mut db = app.store.db.lock().unwrap();
        let tx = db.transaction()?;
        let pause:bool=tx.query_row("SELECT COALESCE(json_extract(json,'$.pause_requested'),0) FROM runs WHERE user_id=? AND id=?",rusqlite::params![uid,parent.id],|r|r.get(0))?;
        if pause {
            parent.pause_requested = true;
            parent.status = "paused".into();
            parent.states.insert(node.id.clone(), "paused".into());
        }
        tx.execute(
            "INSERT INTO runs VALUES(?,?,?)",
            rusqlite::params![child.id, uid, serde_json::to_string(&child)?],
        )?;
        tx.execute(
            "UPDATE runs SET json=? WHERE user_id=? AND id=?",
            rusqlite::params![serde_json::to_string(parent)?, uid, parent.id],
        )?;
        tx.commit()?;
    }
    crate::artifacts::baseline(app, uid, &child)?;
    agents::emit(app, uid, &parent.id, "state", &node.id, "waiting_child");
    notify_parent(app, uid, parent)?;
    agents::emit(
        app,
        uid,
        &rid,
        "created",
        "",
        "Workflow figlio: workspace ereditato, sessioni native separate",
    );
    if parent.pause_requested {
        return Ok(());
    }
    engine::launch(app.clone(), uid, rid)
}
pub fn resume_child(app: &App, uid: i64, parent: &mut Run) -> Result<bool> {
    let Some(call) = active_call(parent).cloned() else {
        return Ok(false);
    };
    let child = app.store.run(uid, &call.run)?;
    if child.status == "completed" {
        let summary = serde_json::to_string_pretty(
            &serde_json::json!({"child_run":child.id,"workflow":child.workflow.name,"status":child.status,"outputs":child.outputs,"artifacts":app.store.artifacts(uid,&child.id)?.iter().map(|a|serde_json::json!({"id":a.id,"path":a.path,"version":a.version})).collect::<Vec<_>>()}),
        )?;
        agents::snapshot(app, uid, parent, &call.node, "", &summary)?;
        parent.outputs.insert(
            call.node.clone(),
            AgentResult {
                status: "completed".into(),
                summary,
                ..Default::default()
            },
        );
        parent.states.insert(call.node, "completed".into());
        engine::route(app, uid, parent, "")?;
        return Ok(parent.status == "completed");
    }
    parent.status = "waiting_child".into();
    parent.states.insert(call.node, "running".into());
    engine::save(app, uid, parent)?;
    if child.pending.is_some() {
        notify_parent(app, uid, &child)?;
    } else if !app.busy.lock().unwrap().contains(&child.id) {
        engine::launch(app.clone(), uid, child.id)?;
    }
    Ok(true)
}
pub fn pause(app: &App, uid: i64, rid: &str) -> Result<()> {
    let r = app.store.run(uid, rid)?;
    let lineage = ancestors(app, uid, &r)?;
    pause_tree(app, uid, &lineage.last().unwrap_or(&r).id)
}
fn pause_tree(app: &App, uid: i64, rid: &str) -> Result<()> {
    let mut r = app.store.run(uid, rid)?;
    r.pause_requested = true;
    if let Some(call) = active_call(&r).cloned() {
        pause_tree(app, uid, &call.run)?;
    }
    if !app.busy.lock().unwrap().contains(rid) && r.pending.is_none() {
        r.status = "paused".into();
    }
    engine::save(app, uid, &mut r)
}

/// Stop the whole active call chain, matching the scope of Pause.
/// Caller holds the lifecycle write lock to exclude resume/decisions/launch.
pub async fn stop(app: &App, uid: i64, rid: &str) -> Result<()> {
    let r = app.store.run(uid, rid)?;
    let lineage = ancestors(app, uid, &r)?;
    let mut next = lineage.last().unwrap_or(&r).id.clone();
    let mut ids = Vec::new();
    loop {
        ids.push(next.clone());
        let control = app.workers.lock().unwrap().get(&next).cloned();
        if let Some(mut control) = control {
            let _ = control.stop.send(true);
            while !*control.done.borrow_and_update() {
                if control.done.changed().await.is_err() {
                    break;
                }
            }
        }
        // Read children only after the parent has stopped: it can no longer
        // launch another child between discovery and cancellation.
        let current = app.store.run(uid, &next)?;
        let Some(call) = active_call(&current) else {
            break;
        };
        next = call.run.clone();
    }
    for id in ids.into_iter().rev() {
        let mut run = app.store.run(uid, &id)?;
        if run.status == "completed" {
            continue;
        }
        run.status = "interrupted".into();
        run.pause_requested = false;
        run.error = Some("Stop richiesto dall’utente. Verifica gli effetti del turno interrotto prima di riprendere.".into());
        for state in run.states.values_mut() {
            if state == "running" {
                *state = "interrupted".into();
            }
        }
        engine::save(app, uid, &mut run)?;
        agents::emit(
            app,
            uid,
            &id,
            "stopped",
            &run.current,
            "Stop immediato richiesto dall’utente",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edge(from: &str, to: &str, kind: &str) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            kind: kind.into(),
            outcome: String::new(),
        }
    }
    fn fixtures() -> (Workflow, Workflow) {
        let child = Workflow {
            execution: Default::default(),
            created: 0,
            id: "child".into(),
            name: "Child review".into(),
            description: String::new(),
            entry: "review".into(),
            nodes: vec![
                Node {
                    id: "review".into(),
                    kind: "Review".into(),
                    title: "Review child".into(),
                    mode: "human".into(),
                    prompt: "Inspect inherited files".into(),
                    ..Default::default()
                },
                Node {
                    id: "read".into(),
                    kind: "Connection".into(),
                    title: "Read inherited file".into(),
                    provider: "local".into(),
                    operation: "read_ticket".into(),
                    location: "from-parent.md".into(),
                    ..Default::default()
                },
            ],
            edges: vec![edge("review", "read", "control")],
        };
        let parent = Workflow {
            execution: ExecutionPolicy {
                instructions: "Parent decision policy".into(),
                ..Default::default()
            },
            created: 0,
            id: "parent".into(),
            name: "Parent call".into(),
            description: String::new(),
            entry: "before".into(),
            nodes: vec![
                Node {
                    id: "source".into(),
                    kind: "Source".into(),
                    title: "Example".into(),
                    source_type: "example".into(),
                    ..Default::default()
                },
                Node {
                    id: "before".into(),
                    kind: "Connection".into(),
                    title: "Read before".into(),
                    provider: "local".into(),
                    operation: "read_ticket".into(),
                    location: "README.md".into(),
                    ..Default::default()
                },
                Node {
                    id: "call".into(),
                    kind: "Workflow".into(),
                    title: "Call child".into(),
                    workflow_id: "child".into(),
                    prompt: "Keep context".into(),
                    ..Default::default()
                },
                Node {
                    id: "after".into(),
                    kind: "Connection".into(),
                    title: "Read after".into(),
                    provider: "local".into(),
                    operation: "read_ticket".into(),
                    location: "from-child.md".into(),
                    ..Default::default()
                },
            ],
            edges: vec![
                edge("source", "before", "context"),
                edge("before", "call", "control"),
                edge("call", "after", "control"),
            ],
        };
        (parent, child)
    }
    async fn wait(app: &App, id: &str, status: &str) -> Run {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let r = app.store.run(1, id).unwrap();
                if r.status == status && !app.busy.lock().unwrap().contains(id) {
                    return r;
                }
                if r.status == "failed" && status != "failed" {
                    panic!("Unexpected failure: {:?}", r.error)
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Timeout waiting {status}: {:?}",
                app.store.run(1, id).unwrap().error
            )
        })
    }
    #[tokio::test]
    async fn immediate_stop_cancels_queued_worker_and_preserves_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = crate::tests::app(&std::fs::canonicalize(dir.path()).unwrap());
        app.slots = std::sync::Arc::new(crate::parallelism::Scheduler::limited(1));
        let held = app.slots.acquire(&app.store, 1).await;
        let (parent, child) = fixtures();
        app.store.save_workflow(1, &child).unwrap();
        let r = engine::create(&app, 1, parent, "stop test".into(), None)
            .await
            .unwrap();
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        assert!(stop(&app, 2, &r.id).await.is_err());
        assert!(app.busy.lock().unwrap().contains(&r.id));
        tokio::time::timeout(std::time::Duration::from_secs(2), stop(&app, 1, &r.id))
            .await
            .unwrap()
            .unwrap();
        let stopped = app.store.run(1, &r.id).unwrap();
        assert_eq!(stopped.status, "interrupted");
        assert_eq!(stopped.sessions.len(), r.sessions.len());
        assert!(!app.busy.lock().unwrap().contains(&r.id));
        assert!(app.workers.lock().unwrap().is_empty());
        drop(held);
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        assert_eq!(app.store.run(1, &r.id).unwrap().status, "interrupted");
    }
    #[tokio::test]
    async fn immediate_stop_from_child_interrupts_family_and_keeps_pending_review() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(&std::fs::canonicalize(dir.path()).unwrap());
        let (parent, child) = fixtures();
        app.store.save_workflow(1, &child).unwrap();
        let r = engine::create(&app, 1, parent, "stop test".into(), None)
            .await
            .unwrap();
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        let parent = wait(&app, &r.id, "waiting_review").await;
        let cid = &parent.children[0].run;
        stop(&app, 1, cid).await.unwrap();
        assert_eq!(app.store.run(1, &r.id).unwrap().status, "interrupted");
        let child = app.store.run(1, cid).unwrap();
        assert_eq!(child.status, "interrupted");
        assert!(child.pending.is_some());
        assert!(app.busy.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn call_shares_files_isolates_sessions_waits_for_review_and_resumes_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = crate::tests::app(&std::fs::canonicalize(dir.path()).unwrap());
        app.slots = std::sync::Arc::new(crate::parallelism::Scheduler::limited(1)); // Parent must release the only slot.
        let (parent, child) = fixtures();
        app.store.save_workflow(1, &child).unwrap();
        app.store.save_workflow(1, &parent).unwrap();
        let mut r = engine::create(&app, 1, parent, "Original request".into(), None)
            .await
            .unwrap();
        r.sessions.insert(
            "author".into(),
            NativeSession {
                runtime: "pi".into(),
                native_id: id(),
                turns: 3,
                model: "test".into(),
            },
        );
        app.store.save_run(1, &r).unwrap();
        std::fs::write(
            PathBuf::from(&r.workspace).join("from-parent.md"),
            "Parent context",
        )
        .unwrap();
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        let p = wait(&app, &r.id, "waiting_review").await;
        assert_eq!(p.states["call"], "waiting_review");
        assert!(p.pending.is_none());
        assert_eq!(p.children.len(), 1);
        let cid = &p.children[0].run;
        let c = wait(&app, cid, "waiting_review").await;
        assert_eq!(c.execution.instructions, "Parent decision policy");
        assert_eq!(c.execution.mode, ExecutionMode::Interactive);
        assert_eq!(c.workspace, r.workspace);
        assert!(c.sessions.is_empty());
        assert!(c.input.contains("Original request"));
        assert!(c.input.contains("Keep context"));
        assert!(c.input.contains("previous_output"));
        assert!(app.store.run(2, cid).is_err());
        assert_eq!(workspace(&app, 1, &c).unwrap(), PathBuf::from(&r.workspace));
        assert!(can_launch(&app, 1, &p).is_ok());
        std::fs::write(
            PathBuf::from(&r.workspace).join("from-child.md"),
            "Child output",
        )
        .unwrap();
        let user = User {
            id: 1,
            email: "test@example.invalid".into(),
        };
        engine::decide(&app, &user, cid, "approve", "Reviewed").unwrap();
        engine::launch(app.clone(), 1, cid.clone()).unwrap();
        let done = wait(&app, &r.id, "completed").await;
        assert_eq!(done.children.len(), 1);
        assert_eq!(done.attempts["call"], 1);
        assert_eq!(done.outputs["after"].summary, "Child output");
        assert!(done.outputs["call"].summary.contains(cid));
        assert_eq!(done.sessions["author"].turns, 3);
        assert!(!app.store.artifacts(1, cid).unwrap().is_empty());
        assert!(!app.store.artifacts(1, &r.id).unwrap().is_empty());
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        assert_eq!(app.store.run(1, &r.id).unwrap().status, "completed");
        assert!(app.busy.lock().unwrap().is_empty());
        // Child cost belongs to the child exactly once; inclusive cost is a separate rollup.
        for (rid, node, cost) in [(&r.id, "before", 3.0), (cid, "read", 2.0)] {
            let usage = serde_json::json!({"id":id(),"node":node,"binding":"fixture","runtime":"fixture","model":"fixture","attempt":1,"at":crate::finops::ms(),"input":10,"output":2,"cache_read":null,"cache_write":null,"reasoning":null,"usd":cost,"llm_ms":null,"calls":1,"tools":0,"source":"unit test"});
            app.store
                .event(1, rid, "usage", node, &usage.to_string())
                .unwrap();
        }
        let report = crate::finops::report(&app, 1).unwrap();
        assert_eq!(report["summary"]["metrics"]["usd"], 5.0);
        let pr = report["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == r.id)
            .unwrap();
        assert_eq!(pr["metrics"]["usd"], 3.0);
        assert_eq!(pr["tree_usd"], 5.0);
        let cr = report["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == *cid)
            .unwrap();
        assert_eq!(cr["tree_usd"], 2.0);
        assert!(crate::deletion::perform(&app, 1, "child", std::slice::from_ref(cid)).is_err());
        let mut ids = vec![r.id.clone(), cid.clone()];
        ids.sort();
        assert_eq!(
            crate::deletion::perform(&app, 1, "parent", &ids).unwrap()["deleted_runs"],
            2
        );
        assert!(app.store.workflow(1, "child").is_ok());
        assert!(!PathBuf::from(&r.workspace).exists());
    }
    #[tokio::test]
    async fn failed_child_retry_reuses_child_run_and_pause_preserves_review() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(&std::fs::canonicalize(dir.path()).unwrap());
        let (parent, child) = fixtures();
        app.store.save_workflow(1, &child).unwrap();
        let r = engine::create(&app, 1, parent, String::new(), None)
            .await
            .unwrap();
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        let p = wait(&app, &r.id, "waiting_review").await;
        let cid = p.children[0].run.clone();
        pause(&app, 1, &r.id).unwrap();
        assert!(app.store.run(1, &cid).unwrap().pending.is_some());
        assert!(can_launch(&app, 1, &app.store.run(1, &cid).unwrap()).is_err());
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        wait(&app, &r.id, "waiting_review").await;
        let user = User {
            id: 1,
            email: "test@example.invalid".into(),
        };
        engine::decide(&app, &user, &cid, "approve", "").unwrap();
        engine::launch(app.clone(), 1, cid.clone()).unwrap();
        let failed = wait(&app, &r.id, "failed").await;
        assert_eq!(failed.children.len(), 1);
        std::fs::write(PathBuf::from(&r.workspace).join("from-parent.md"), "Fixed").unwrap();
        std::fs::write(
            PathBuf::from(&r.workspace).join("from-child.md"),
            "Fixed child",
        )
        .unwrap();
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        let done = wait(&app, &r.id, "completed").await;
        assert_eq!(done.children[0].run, cid);
        assert_eq!(done.attempts["call"], 1);
    }
    #[tokio::test]
    async fn nested_calls_complete_with_one_slot_and_survive_restart_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = crate::tests::app(&std::fs::canonicalize(dir.path()).unwrap());
        app.slots = std::sync::Arc::new(crate::parallelism::Scheduler::limited(1));
        let (parent, mut middle) = fixtures();
        let mut leaf = middle.clone();
        leaf.id = "leaf".into();
        middle.nodes = vec![Node {
            id: "nested".into(),
            kind: "Workflow".into(),
            title: "Nested call".into(),
            workflow_id: "leaf".into(),
            ..Default::default()
        }];
        middle.entry = "nested".into();
        middle.edges.clear();
        app.store.save_workflow(1, &middle).unwrap();
        app.store.save_workflow(1, &leaf).unwrap();
        let r = engine::create(&app, 1, parent, String::new(), None)
            .await
            .unwrap();
        for name in ["from-parent.md", "from-child.md"] {
            std::fs::write(PathBuf::from(&r.workspace).join(name), "Inherited").unwrap();
        }
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        let p = wait(&app, &r.id, "waiting_review").await;
        let mid = app.store.run(1, &p.children[0].run).unwrap();
        let lid = mid.children[0].run.clone();
        wait(&app, &lid, "waiting_review").await;
        app.busy.lock().unwrap().insert(lid.clone());
        assert!(engine::launch(app.clone(), 1, r.id.clone()).is_err());
        app.busy.lock().unwrap().remove(&lid);
        // Recovery preserves call identities and pending human review, rather than re-invoking.
        let mut interrupted = app.store.run(1, &r.id).unwrap();
        interrupted.status = "waiting_child".into();
        app.store.save_run(1, &interrupted).unwrap();
        app.store.interrupt_running().unwrap();
        assert_eq!(app.store.run(1, &r.id).unwrap().status, "interrupted");
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        wait(&app, &r.id, "waiting_review").await;
        let user = User {
            id: 1,
            email: "test@example.invalid".into(),
        };
        engine::decide(&app, &user, &lid, "approve", "").unwrap();
        engine::launch(app.clone(), 1, lid.clone()).unwrap();
        let done = wait(&app, &r.id, "completed").await;
        assert_eq!(done.children.len(), 1);
        assert_eq!(done.children[0].run, mid.id);
        assert_eq!(app.store.run(1, &mid.id).unwrap().children[0].run, lid);
        assert_eq!(app.store.run(1, &lid).unwrap().status, "completed");
        // A child with no human wait can complete before its caller releases its slot.
        leaf.nodes.retain(|n| n.id == "read");
        leaf.nodes[0].location = "README.md".into();
        leaf.entry = "read".into();
        leaf.edges.clear();
        app.store.save_workflow(1, &leaf).unwrap();
        let (parent, _) = fixtures();
        let r = engine::create(&app, 1, parent, String::new(), None)
            .await
            .unwrap();
        std::fs::write(PathBuf::from(&r.workspace).join("from-child.md"), "Done").unwrap();
        engine::launch(app.clone(), 1, r.id.clone()).unwrap();
        assert_eq!(wait(&app, &r.id, "completed").await.children.len(), 1);
    }
    #[test]
    fn rejects_missing_cross_user_recursive_and_deep_calls() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(&std::fs::canonicalize(dir.path()).unwrap());
        let (parent, mut child) = fixtures();
        assert!(validate_tree(&app, 1, &parent).is_err());
        app.store.save_workflow(2, &child).unwrap();
        assert!(validate_tree(&app, 1, &parent).is_err());
        app.store.save_workflow(1, &child).unwrap();
        validate_tree(&app, 1, &parent).unwrap();
        child.nodes = vec![Node {
            id: "call".into(),
            kind: "Workflow".into(),
            title: "Recursive".into(),
            workflow_id: parent.id.clone(),
            ..Default::default()
        }];
        child.entry = "call".into();
        child.edges.clear();
        app.store.save_workflow(1, &parent).unwrap();
        app.store.save_workflow(1, &child).unwrap();
        assert!(validate_tree(&app, 1, &parent).is_err());
        for i in 0..6 {
            let w = Workflow {
                execution: Default::default(),
                created: 0,
                id: format!("depth{i}"),
                name: "Depth".into(),
                description: String::new(),
                entry: "call".into(),
                nodes: vec![if i == 5 {
                    Node {
                        id: "call".into(),
                        kind: "Review".into(),
                        mode: "human".into(),
                        ..Default::default()
                    }
                } else {
                    Node {
                        id: "call".into(),
                        kind: "Workflow".into(),
                        workflow_id: format!("depth{}", i + 1),
                        ..Default::default()
                    }
                }],
                edges: vec![],
            };
            app.store.save_workflow(1, &w).unwrap();
        }
        assert!(validate_tree(&app, 1, &app.store.workflow(1, "depth0").unwrap()).is_err());
    }
    #[tokio::test]
    async fn deleting_direct_parent_and_child_preserves_all_source_files() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(&std::fs::canonicalize(dir.path()).unwrap());
        let original = app.sources.join("material");
        std::fs::create_dir_all(original.join("empty")).unwrap();
        std::fs::write(original.join("keep.txt"), "keep original").unwrap();
        let (mut parent_w, child_w) = fixtures();
        let source = parent_w
            .nodes
            .iter_mut()
            .find(|n| n.kind == "Source")
            .unwrap();
        source.source_type = "folder".into();
        source.source_mode = "direct".into();
        source.location = "material".into();
        app.store.save_workflow(1, &parent_w).unwrap();
        app.store.save_workflow(1, &child_w).unwrap();
        let mut parent = engine::create(&app, 1, parent_w.clone(), "test".into(), None)
            .await
            .unwrap();
        parent.current = "call".into();
        parent.pause_requested = true;
        app.store.save_run(1, &parent).unwrap();
        let call = parent.workflow.node("call").unwrap().clone();
        invoke(&app, 1, &mut parent, &call).unwrap();
        let child_id = parent.children.last().unwrap().run.clone();
        let mut child = app.store.run(1, &child_id).unwrap();
        // Both are paused without launching any agent; deletion cascades to the child.
        parent.status = "paused".into();
        child.status = "paused".into();
        app.store.save_run(1, &parent).unwrap();
        app.store.save_run(1, &child).unwrap();
        std::fs::write(original.join("from-child.md"), "child output").unwrap();
        crate::deletion::perform(
            &app,
            1,
            &parent_w.id,
            &[parent.id.clone(), child.id.clone()],
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(original.join("keep.txt")).unwrap(),
            "keep original"
        );
        assert_eq!(
            std::fs::read_to_string(original.join("from-child.md")).unwrap(),
            "child output"
        );
        assert!(original.join("empty").is_dir());
        assert!(app.store.run(1, &parent.id).is_err());
        assert!(app.store.run(1, &child.id).is_err());
    }
}
