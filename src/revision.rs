//! Fork a stopped run after previewing the reusable execution prefix.
use crate::{App, agents, model::*, store::hash};
use anyhow::{Context, Result, ensure};
use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};
use std::{io::Read, os::unix::fs::PermissionsExt};

fn executable(n: &Node) -> bool {
    !["Source", "Agent", "MCP", "RAG", "Comment"].contains(&n.kind.as_str())
}
fn signature(n: &Node) -> Value {
    let mut v = serde_json::to_value(n).unwrap();
    for key in [
        "title", "phase", "x", "y", "width", "height", "text", "markdown",
    ] {
        v.as_object_mut().unwrap().remove(key);
    }
    v
}
fn incoming(w: &Workflow, id: &str) -> Vec<String> {
    let mut edges: Vec<_> = w
        .edges
        .iter()
        .filter(|e| e.to == id && e.kind != "rework")
        .map(|e| format!("{}:{}:{}", e.from, e.kind, e.outcome))
        .collect();
    edges.sort();
    edges
}
fn dependent(w: &Workflow, from: &str, to: &Node) -> bool {
    w.edges
        .iter()
        .any(|e| e.from == from && e.to == to.id && e.kind != "rework")
        || to.agent == from && ["Task", "Review", "Gate", "Memory"].contains(&to.kind.as_str())
}
fn propagate(w: &Workflow, affected: &mut HashSet<String>) {
    loop {
        let added: Vec<_> = w
            .nodes
            .iter()
            .filter(|n| {
                n.kind != "Comment"
                    && !affected.contains(&n.id)
                    && affected.iter().any(|id| dependent(w, id, n))
            })
            .map(|n| n.id.clone())
            .collect();
        if added.is_empty() {
            break;
        }
        affected.extend(added);
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Step {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}
fn step(n: &Node, reason: &str) -> Step {
    Step {
        id: n.id.clone(),
        title: n.title.clone(),
        kind: n.kind.clone(),
        reason: reason.into(),
        summary: None,
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub token: String,
    pub base_run: String,
    pub restart: Option<String>,
    pub candidates: Vec<Step>,
    pub kept: Vec<Step>,
    pub rerun: Vec<Step>,
    pub removed: Vec<Step>,
    pub artifacts: Vec<Value>,
    pub workspace_files: Vec<String>,
}
fn available(app: &App, uid: i64, r: &Run) -> Result<()> {
    let busy = app.busy.lock().unwrap();
    ensure!(
        !busy.contains(&r.id)
            && [
                "paused",
                "failed",
                "interrupted",
                "completed",
                "waiting_review",
                "waiting_user"
            ]
            .contains(&r.status.as_str()),
        "Metti la run in pausa e attendi la fine del nodo attivo prima di modificarla"
    );
    ensure!(
        r.parent.is_none(),
        "Modifica il workflow principale; le run figlie restano collegate al loro chiamante"
    );
    crate::subworkflows::can_launch_reserved(app, uid, r, &busy)?;
    let mut queue: Vec<_> = r.children.iter().map(|c| c.run.clone()).collect();
    let mut seen = HashSet::new();
    while let Some(id) = queue.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let child = app.store.run(uid, &id)?;
        ensure!(
            !busy.contains(&id)
                && ["completed", "failed", "interrupted"].contains(&child.status.as_str())
                && child.pending.is_none(),
            "Completa le attività e le richieste dei workflow figli prima di modificare la run"
        );
        queue.extend(child.children.iter().map(|c| c.run.clone()));
    }
    Ok(())
}
pub fn is_available(app: &App, uid: i64, r: &Run) -> bool {
    available(app, uid, r).is_ok()
}
fn sources(w: &Workflow) -> Vec<Value> {
    w.nodes
        .iter()
        .filter(|n| n.kind == "Source")
        .map(signature)
        .collect()
}
fn source_roots(r: &Run) -> Vec<(String, PathBuf)> {
    r.workflow
        .nodes
        .iter()
        .filter(|n| n.kind == "Source")
        .enumerate()
        .map(|(i, n)| {
            let path = crate::source_access::source_path(r, i, n);
            (
                n.id.clone(),
                if Path::new(&path).is_absolute() {
                    PathBuf::from(path)
                } else {
                    Path::new(&r.workspace).join(path)
                },
            )
        })
        .collect()
}
fn files(root: &Path, prefix: &str, output: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if [".git", "target", "node_modules"].contains(&name.as_str()) {
            continue;
        }
        let meta = std::fs::symlink_metadata(entry.path())?;
        ensure!(
            !meta.is_symlink(),
            "La copia del workspace non supporta link simbolici: {name}"
        );
        let path = format!("{prefix}{name}");
        if meta.is_dir() {
            files(&entry.path(), &format!("{path}/"), output)?;
        } else {
            ensure!(meta.is_file(), "File non supportato: {path}");
            output.push(path);
        }
        ensure!(
            output.len() <= 20000,
            "Troppi file per l’anteprima della ripartenza (massimo 20000)"
        );
    }
    Ok(())
}
fn fingerprint(r: &Run) -> Result<String> {
    fn scan(root: &Path, records: &mut Vec<(String, String, u32)>, prefix: &str) -> Result<()> {
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if [".git", "target", "node_modules"].contains(&name.as_str()) {
                continue;
            }
            let meta = std::fs::symlink_metadata(entry.path())?;
            ensure!(!meta.is_symlink(), "Link simbolico non supportato: {name}");
            let path = format!("{prefix}{name}");
            if meta.is_dir() {
                scan(&entry.path(), records, &format!("{path}/"))?;
            } else {
                ensure!(meta.is_file(), "File non supportato: {path}");
                let mut file = std::fs::File::open(entry.path())?;
                let mut hasher = Sha256::new();
                let mut buffer = [0u8; 16384];
                loop {
                    let n = file.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                records.push((
                    path,
                    hex::encode(hasher.finalize()),
                    meta.permissions().mode() & 0o777,
                ));
            }
            ensure!(
                records.len() <= 20000,
                "Troppi file per verificare la ripartenza (massimo 20000)"
            );
        }
        Ok(())
    }
    let mut records = Vec::new();
    scan(Path::new(&r.workspace), &mut records, "")?;
    for (id, root) in source_roots(r).into_iter().skip(1) {
        scan(&root, &mut records, &format!("secondary:{id}/"))?;
    }
    records.sort();
    Ok(hash(serde_json::to_vec(&records)?))
}
// The default restart is the first non-reusable node on the previously successful route.
// An explicit restart may go earlier on that prefix, never skip an invalid prerequisite.
fn plan_graph(r: &Run, w: &Workflow, requested: Option<&str>) -> Result<Plan> {
    ensure!(
        w.id == r.workflow.id,
        "La bozza deve appartenere allo stesso workflow della run"
    );
    ensure!(
        sources(w) == sources(&r.workflow),
        "Per conservare il lavoro mantieni le Source originali (tipo, percorso e modalità). Per cambiare sorgente avvia una nuova run."
    );
    let policy_changed =
        serde_json::to_value(&w.execution)? != serde_json::to_value(&r.workflow.execution)?;
    let mut affected = HashSet::new();
    let mut reasons = BTreeMap::new();
    for n in w.nodes.iter().filter(|n| n.kind != "Comment") {
        let old = r.workflow.nodes.iter().find(|old| old.id == n.id);
        let reason = match old {
            None => Some("new"),
            Some(old)
                if signature(old) != signature(n)
                    || incoming(&r.workflow, &n.id) != incoming(w, &n.id) =>
            {
                Some("changed")
            }
            _ if policy_changed && executable(n) => Some("policy"),
            _ if executable(n)
                && (r.states.get(&n.id).is_none_or(|s| s != "completed")
                    || r.outputs.get(&n.id).is_none_or(|o| o.status != "completed")) =>
            {
                Some("unfinished")
            }
            // External effects and CI evidence belong to their original run/repository.
            _ if n.kind == "Connection"
                && (crate::connectors::is_write(n)
                    || n.operation == "test"
                    || n.operation == "read_ci") =>
            {
                Some("evidence")
            }
            _ => None,
        };
        if let Some(reason) = reason {
            affected.insert(n.id.clone());
            reasons.insert(n.id.clone(), reason);
        }
    }
    propagate(w, &mut affected);
    let mut cursor = Some(w.entry.clone());
    let mut candidates = Vec::new();
    let mut route = Vec::new();
    let mut seen = HashSet::new();
    let mut default = None;
    while let Some(id) = cursor {
        ensure!(
            seen.insert(id.clone()),
            "Percorso di ripartenza ambiguo: scegli una nuova esecuzione"
        );
        let n = w.node(&id)?;
        candidates.push(step(n, "checkpoint"));
        route.push(id.clone());
        if affected.contains(&id) {
            default = Some(id);
            break;
        }
        let output = r
            .outputs
            .get(&id)
            .context("Risultato precedente non disponibile")?;
        let outcome = if n.kind == "Gate" {
            output.decision.as_str()
        } else {
            ""
        };
        cursor = w
            .next(&id, outcome)
            .filter(|e| e.kind == "control")
            .map(|e| e.to.clone());
    }
    let restart = requested.map(str::to_string).or(default);
    if let Some(id) = &restart {
        ensure!(
            route.contains(id),
            "Non puoi saltare nodi nuovi, modificati o non completati: scegli un punto precedente"
        );
        affected.insert(id.clone());
        reasons.entry(id.clone()).or_insert("restart");
        propagate(w, &mut affected);
    }
    let mut kept: Vec<Step> = w
        .nodes
        .iter()
        .filter(|n| executable(n) && !affected.contains(&n.id))
        .map(|n| {
            let mut s = step(n, "compatible");
            s.summary = r
                .outputs
                .get(&n.id)
                .map(|o| o.summary.chars().take(2000).collect());
            s
        })
        .collect();
    let mut rerun: Vec<Step> = w
        .nodes
        .iter()
        .filter(|n| executable(n) && affected.contains(&n.id))
        .map(|n| step(n, reasons.get(&n.id).copied().unwrap_or("dependency")))
        .collect();
    let mut degrees: BTreeMap<String, usize> = w
        .nodes
        .iter()
        .map(|n| {
            (
                n.id.clone(),
                w.edges
                    .iter()
                    .filter(|e| e.to == n.id && e.kind == "control")
                    .count(),
            )
        })
        .collect();
    let mut queue: std::collections::VecDeque<_> = w
        .nodes
        .iter()
        .filter(|n| degrees[&n.id] == 0)
        .map(|n| n.id.clone())
        .collect();
    let mut order = BTreeMap::new();
    while let Some(id) = queue.pop_front() {
        order.insert(id.clone(), order.len());
        for e in w
            .edges
            .iter()
            .filter(|e| e.from == id && e.kind == "control")
        {
            let degree = degrees.get_mut(&e.to).unwrap();
            *degree -= 1;
            if *degree == 0 {
                queue.push_back(e.to.clone());
            }
        }
    }
    kept.sort_by_key(|s| order.get(&s.id).copied().unwrap_or(usize::MAX));
    rerun.sort_by_key(|s| order.get(&s.id).copied().unwrap_or(usize::MAX));
    let removed = r
        .workflow
        .nodes
        .iter()
        .filter(|n| n.kind != "Comment" && !w.nodes.iter().any(|x| x.id == n.id))
        .map(|n| step(n, "removed"))
        .collect();
    Ok(Plan {
        token: String::new(),
        base_run: r.id.clone(),
        restart,
        candidates,
        kept,
        rerun,
        removed,
        artifacts: Vec::new(),
        workspace_files: Vec::new(),
    })
}
fn workflow_tree(app: &App, uid: i64, w: &Workflow, depth: usize) -> Result<Value> {
    ensure!(
        depth < crate::subworkflows::MAX_DEPTH,
        "Troppi livelli di workflow annidati"
    );
    let mut children = Vec::new();
    for n in w.nodes.iter().filter(|n| n.kind == "Workflow") {
        let child = app.store.workflow(uid, &n.workflow_id)?;
        children.push(json!({"node":n.id,"workflow":workflow_tree(app,uid,&child,depth+1)?}));
    }
    Ok(json!({"workflow":w,"children":children}))
}
fn call_compatible(app: &App, uid: i64, r: &Run, depth: usize) -> Result<bool> {
    if depth >= crate::subworkflows::MAX_DEPTH {
        return Ok(false);
    }
    let current = app.store.workflow(uid, &r.workflow.id)?;
    if serde_json::to_value(&current)? != serde_json::to_value(&r.workflow)? {
        return Ok(false);
    }
    for n in
        r.workflow.nodes.iter().filter(|n| {
            n.kind == "Workflow" && r.states.get(&n.id).is_some_and(|s| s == "completed")
        })
    {
        if let Some(call) = r.children.iter().rev().find(|c| c.node == n.id) {
            if !call_compatible(app, uid, &app.store.run(uid, &call.run)?, depth + 1)? {
                return Ok(false);
            }
        } else {
            let child = app.store.workflow(uid, &n.workflow_id)?;
            let proof = hash(serde_json::to_vec(&workflow_tree(
                app,
                uid,
                &child,
                depth + 1,
            )?)?);
            if r.context.get(&format!("inherited-workflow:{}", n.id)) != Some(&proof) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
pub fn preview(
    app: &App,
    uid: i64,
    rid: &str,
    w: &Workflow,
    requested: Option<&str>,
) -> Result<Plan> {
    let r = app.store.run(uid, rid)?;
    available(app, uid, &r)?;
    crate::subworkflows::validate_tree(app, uid, w)?;
    let mut invalid = r.clone();
    // A called workflow can change independently of its caller node.
    for n in w.nodes.iter().filter(|n| n.kind == "Workflow") {
        let current = app.store.workflow(uid, &n.workflow_id)?;
        let compatible = if let Some(call) = r.children.iter().rev().find(|call| call.node == n.id)
        {
            call_compatible(app, uid, &app.store.run(uid, &call.run)?, 1)?
        } else {
            false
        };
        let proof = hash(serde_json::to_vec(&workflow_tree(app, uid, &current, 1)?)?);
        let inherited = r.context.get(&format!("inherited-workflow:{}", n.id)) == Some(&proof);
        if !compatible && !inherited {
            invalid.states.insert(n.id.clone(), "queued".into());
        }
    }
    let mut p = plan_graph(&invalid, w, requested)?;
    finish_plan(app, uid, &r, w, &mut p)?;
    Ok(p)
}
fn finish_plan(app: &App, uid: i64, r: &Run, w: &Workflow, p: &mut Plan) -> Result<()> {
    let kept: HashSet<_> = p.kept.iter().map(|s| s.id.as_str()).collect();
    p.artifacts = app
        .store
        .artifacts(uid, &r.id)?
        .iter()
        .filter(|a| kept.contains(a.node.as_str()))
        .map(|a| json!({"node":a.node,"path":a.path,"version":a.version}))
        .collect();
    files(Path::new(&r.workspace), "", &mut p.workspace_files)?;
    for (id, root) in source_roots(r).into_iter().skip(1) {
        // Secondary copied sources are already inside the primary workspace.
        if !root.starts_with(&r.workspace) {
            files(
                &root,
                &format!(".nodo/sources/{id}/"),
                &mut p.workspace_files,
            )?;
        }
    }
    p.workspace_files.sort();
    p.workspace_files.dedup();
    let saved = app.store.workflow(uid, &w.id)?;
    p.token = hash(serde_json::to_vec(&(
        r,
        w,
        &p,
        fingerprint(r)?,
        saved,
        app.store.settings(uid)?,
        workflow_tree(app, uid, w, 0)?,
    ))?);
    Ok(())
}
pub async fn create(
    app: &App,
    uid: i64,
    rid: &str,
    w: Workflow,
    requested: Option<&str>,
    token: &str,
) -> Result<(Run, bool)> {
    ensure!(!token.is_empty(), "Apri l’anteprima prima di continuare");
    // Recover a successful confirmation retry without creating or launching another run.
    let existing:Option<String>=app.store.db.lock().unwrap().query_row(
        "SELECT json FROM runs WHERE user_id=? AND json_extract(json,'$.context.revision-token')=? AND json_extract(json,'$.context.revision-base')=?",
        rusqlite::params![uid,token,rid],|row|row.get(0)).optional()?;
    if let Some(raw) = existing {
        return Ok((serde_json::from_str(&raw)?, false));
    }
    let p = preview(app, uid, rid, &w, requested)?;
    ensure!(
        p.token == token,
        "La run, i file o il workflow sono cambiati. Aggiorna l’anteprima prima di continuare."
    );
    let restart = p
        .restart
        .as_ref()
        .context("Scegli un nodo da cui ripartire nell’anteprima")?;
    let old = app.store.run(uid, rid)?;
    let proof = fingerprint(&old)?;
    let new_id = id();
    let dest = agents::user_home(app, uid)?
        .join("workspaces")
        .join(&new_id);
    let result: Result<Run> = async {
        agents::copy_tree(Path::new(&old.workspace), &dest)?;
        let mut context = BTreeMap::new();
        for (index,(source, root)) in source_roots(&old).into_iter().enumerate() {
            if index == 0 { context.insert(format!("source-path:{source}"), ".".into()); continue; }
            let target = dest.join(".nodo/sources").join(&source);
            if target.exists() { std::fs::remove_dir_all(&target)?; }
            agents::copy_tree(&root, &target)?;
            context.insert(format!("source-path:{source}"), format!(".nodo/sources/{source}"));
        }
        ensure!(fingerprint(&old)? == proof, "I file sono cambiati durante la copia: aggiorna l’anteprima");
        agents::own_tree(app, uid, &dest)?;
        agents::tool(app, uid, &dest, "git", &["init","-b","main"],30).await?;
        agents::tool(app, uid, &dest, "git", &["add","--all"],30).await?;
        agents::tool(app, uid, &dest, "git", &["-c","core.hooksPath=/dev/null","-c","user.name=Nodo","-c","user.email=nodo@local","commit","--allow-empty","-m","Inherited run workspace"],30).await?;
        let keep: HashSet<_> = p.kept.iter().map(|s| s.id.as_str()).collect();
        let mut workflow = w.clone();
        for n in &mut workflow.nodes { if n.kind == "Source" && n.source_type == "folder" { n.source_mode = "copy".into(); } }
        let settings = app.store.settings(uid)?;
        for n in &mut workflow.nodes { if n.kind == "Agent" && n.runtime.is_empty() { n.runtime = settings.default_runtime.clone(); } }
        let mut states = BTreeMap::new();
        for n in workflow.nodes.iter().filter(|n| n.kind != "Comment") { states.insert(n.id.clone(), if !executable(n) || keep.contains(n.id.as_str()) {"completed"} else {"queued"}.into()); }
        let outputs = old.outputs.iter().filter(|(id,_)| keep.contains(id.as_str())).map(|(id,o)|(id.clone(),o.clone())).collect();
        for (key,value) in &old.context {
            if key == "user-note" { context.insert(key.clone(),value.clone()); }
        }
        for node in workflow.nodes.iter().filter(|n|n.kind=="Workflow" && keep.contains(n.id.as_str())) {
            context.insert(format!("inherited-workflow:{}",node.id),hash(serde_json::to_vec(&workflow_tree(app,uid,&app.store.workflow(uid,&node.workflow_id)?,1)?)?));
        }
        context.insert("revision-base".into(), old.id.clone()); context.insert("revision-token".into(), token.into());
        context.insert("source-run".into(), json!({"run_id":old.id,"inherited_results":p.kept,"note":"Inherited successful prefix; historical approvals only. Files copied into a new workspace. No native sessions or external-operation authorizations reused."}).to_string());
        let mut last_node = None; let mut cursor = Some(workflow.entry.clone()); let mut last_task = None;
        while let Some(node) = cursor {
            if &node == restart { break; }
            let n = workflow.node(&node)?;
            if n.kind == "Task" { last_task = Some(node.clone()); }
            if n.kind == "Memory" && let Some(output) = old.outputs.get(&node) { context.insert(format!("handoff:{}",n.target), output.summary.clone()); }
            last_node = Some(node.clone());
            let outcome = if n.kind == "Gate" { old.outputs.get(&node).map(|o|o.decision.as_str()).unwrap_or("") } else { "" };
            cursor = workflow.next(&node,outcome).map(|e|e.to.clone());
        }
        let active_edge = last_node.as_ref().and_then(|from| workflow.edges.iter().find(|e| &e.from == from && &e.to == restart && e.kind == "control")).cloned();
        let mut execution = workflow.execution.clone(); if execution.runtime.is_empty() { execution.runtime = settings.default_runtime; }
        let r = Run { id:new_id.clone(),workflow,status:"paused".into(),current:restart.clone(),created:now(),updated:now(),workspace:dest.to_string_lossy().into(),states,outputs,context,input:old.input.clone(),execution,parent:None,children:Vec::new(),sessions:BTreeMap::new(),pending:None,active_edge,pause_requested:false,attempts:BTreeMap::new(),reworks:BTreeMap::new(),last_task,last_node,error:None };
        let mut artifacts = Vec::new();
        for mut a in app.store.artifacts(uid,rid)?.into_iter().filter(|a|keep.contains(a.node.as_str())) { crate::artifacts::rebase(app,uid,&old,&r,&mut a);a.id=id();a.run=new_id.clone();artifacts.push(a); }
        let mut db = app.store.db.lock().unwrap(); let tx=db.transaction()?;
        tx.execute("INSERT INTO runs VALUES(?,?,?)",rusqlite::params![r.id,uid,serde_json::to_string(&r)?])?;
        tx.execute("INSERT INTO workflows VALUES(?,?,?) ON CONFLICT(id,user_id) DO UPDATE SET json=json_set(excluded.json,'$.created',COALESCE(json_extract(workflows.json,'$.created'),0))",rusqlite::params![w.id,uid,serde_json::to_string(&w)?])?;
        for a in artifacts { tx.execute("INSERT INTO artifacts VALUES(?,?,?,?,?)",rusqlite::params![a.id,uid,a.run,a.node,serde_json::to_string(&a)?])?; }
        tx.commit()?; drop(db);
        crate::artifacts::baseline(app, uid, &r)?;
        Ok(r)
    }.await;
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&dest);
    }
    let r = result?;
    agents::emit(
        app,
        uid,
        &r.id,
        "revision",
        &r.current,
        &format!(
            "Ripartenza dalla run {}: {} risultati mantenuti, {} nodi da eseguire",
            old.id,
            p.kept.len(),
            p.rerun.len()
        ),
    );
    Ok((r, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn fixture() -> (tempfile::TempDir, App, Run) {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let w:Workflow=serde_json::from_value(json!({"id":"revision-workflow","name":"Revision test","description":"","entry":"a","nodes":[
            {"id":"source","kind":"Source","title":"Source","source_type":"example"},
            {"id":"agent","kind":"Agent","title":"Agent","runtime":"codex"},
            {"id":"a","kind":"Task","title":"Task A","agent":"agent","prompt":"Do A"},
            {"id":"review","kind":"Review","title":"Review","mode":"human"}],
            "edges":[{"from":"source","to":"agent","kind":"context"},{"from":"agent","to":"a","kind":"context"},{"from":"a","to":"review","kind":"control"}]})).unwrap();
        app.store.save_workflow(1, &w).unwrap();
        let mut r = crate::engine::create(&app, 1, w, "Original input".into(), None)
            .await
            .unwrap();
        for id in ["a", "review"] {
            r.states.insert(id.into(), "completed".into());
            r.outputs.insert(
                id.into(),
                AgentResult {
                    status: "completed".into(),
                    summary: format!("{id} result"),
                    ..Default::default()
                },
            );
        }
        r.status = "completed".into();
        r.current = "review".into();
        app.store.save_run(1, &r).unwrap();
        (dir, app, r)
    }
    fn insert(r: &Run) -> Workflow {
        let mut w = r.workflow.clone();
        let mut b = w.node("a").unwrap().clone();
        b.id = "b".into();
        b.title = "Task B".into();
        b.prompt = "Do B".into();
        w.nodes.push(b);
        w.edges.retain(|e| !(e.from == "a" && e.to == "review"));
        for (from, to, kind) in [
            ("a", "b", "control"),
            ("b", "review", "control"),
            ("agent", "b", "context"),
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
    #[tokio::test]
    async fn insertion_keeps_prefix_and_never_skips_new_prerequisites() {
        let (_dir, app, r) = fixture().await;
        let w = insert(&r);
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        assert_eq!(p.restart.as_deref(), Some("b"));
        assert_eq!(
            p.kept.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            vec!["a"]
        );
        assert_eq!(
            p.rerun.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            vec!["b", "review"]
        );
        assert!(preview(&app, 1, &r.id, &w, Some("review")).is_err());
        let p = preview(&app, 1, &r.id, &w, Some("a")).unwrap();
        assert!(p.kept.is_empty());
        assert_eq!(p.rerun.len(), 3);
    }
    #[tokio::test]
    async fn configuration_dependencies_and_policy_invalidate_results_but_layout_does_not() {
        let (_dir, app, r) = fixture().await;
        let mut w = r.workflow.clone();
        w.nodes[2].x = 900.;
        w.nodes[2].title = "Renamed".into();
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        assert!(p.restart.is_none());
        assert_eq!(p.kept.len(), 2);
        w.nodes[1].model = "changed-model".into();
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        assert_eq!(p.restart.as_deref(), Some("a"));
        assert!(p.kept.is_empty());
        let mut w = r.workflow.clone();
        w.execution.max_decisions += 1;
        assert!(preview(&app, 1, &r.id, &w, None).unwrap().kept.is_empty());
    }
    #[tokio::test]
    async fn source_changes_and_busy_or_foreign_runs_are_rejected() {
        let (_dir, app, r) = fixture().await;
        let mut w = insert(&r);
        w.nodes[0].location = "other".into();
        assert!(preview(&app, 1, &r.id, &w, None).is_err());
        let w = insert(&r);
        assert!(preview(&app, 2, &r.id, &w, None).is_err());
        app.busy.lock().unwrap().insert(r.id.clone());
        assert!(preview(&app, 1, &r.id, &w, None).is_err());
    }
    #[tokio::test]
    async fn stale_preview_rejects_file_and_draft_changes_without_creating_runs() {
        let (_dir, app, r) = fixture().await;
        let w = insert(&r);
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        std::fs::write(
            Path::new(&r.workspace).join("new-file.txt"),
            "changed after preview",
        )
        .unwrap();
        assert!(
            create(&app, 1, &r.id, w.clone(), None, &p.token)
                .await
                .is_err()
        );
        assert_eq!(app.store.runs(1).unwrap().len(), 1);
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        let mut changed = w;
        changed.nodes.last_mut().unwrap().prompt = "Changed after preview".into();
        assert!(
            create(&app, 1, &r.id, changed, None, &p.token)
                .await
                .is_err()
        );
        assert_eq!(app.store.runs(1).unwrap().len(), 1);
    }
    #[tokio::test]
    async fn fork_copies_work_and_compatible_artifacts_but_not_sessions_or_authorizations() {
        let (_dir, app, mut r) = fixture().await;
        std::fs::write(
            Path::new(&r.workspace).join("finished-work.txt"),
            "Work already done",
        )
        .unwrap();
        r.status = "waiting_review".into();
        r.pending = Some(Pending {
            id: id(),
            proposal: None,
            kind: "review".into(),
            node: "review".into(),
            questions: vec![],
            resume_task: None,
        });
        r.states.insert("review".into(), "waiting_review".into());
        r.outputs.remove("review");
        r.sessions.insert(
            "agent".into(),
            NativeSession {
                runtime: "codex".into(),
                native_id: "old-session".into(),
                turns: 4,
                model: "".into(),
            },
        );
        for key in [
            "effect:b",
            "human-approved:review",
            "test-proof:a",
            "feedback:b",
            "answer:b",
        ] {
            r.context.insert(key.into(), "old authorization".into());
        }
        app.store.save_run(1, &r).unwrap();
        let original = serde_json::to_value(&r).unwrap();
        for node in ["a", "review"] {
            app.store
                .add_artifact(
                    1,
                    &Artifact {
                        file: None,
                        id: id(),
                        run: r.id.clone(),
                        node: node.into(),
                        path: format!("{node}.txt"),
                        version: 1,
                        hash: "hash".into(),
                        content: "saved output".into(),
                        session: "old-session".into(),
                        turn: 1,
                        created: now(),
                    },
                )
                .unwrap();
        }
        let w = insert(&r);
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        assert_eq!(p.artifacts.len(), 1);
        let (fork, created) = create(&app, 1, &r.id, w.clone(), None, &p.token)
            .await
            .unwrap();
        assert!(created);
        assert_ne!(fork.workspace, r.workspace);
        assert_eq!(fork.current, "b");
        assert_eq!(fork.last_node.as_deref(), Some("a"));
        assert_eq!(fork.last_task.as_deref(), Some("a"));
        assert_eq!(fork.outputs.len(), 1);
        assert_eq!(fork.states["a"], "completed");
        assert_eq!(fork.states["review"], "queued");
        assert!(fork.pending.is_none());
        assert!(fork.sessions.is_empty());
        assert!(fork.attempts.is_empty());
        assert_eq!(
            std::fs::read_to_string(Path::new(&fork.workspace).join("finished-work.txt")).unwrap(),
            "Work already done"
        );
        assert!(!fork.context.keys().any(|k| k.starts_with("effect:")
            || k.starts_with("test-proof:")
            || k.starts_with("human-approved:")
            || k.starts_with("answer:")));
        assert_eq!(
            serde_json::to_value(app.store.run(1, &r.id).unwrap()).unwrap(),
            original
        );
        let artifacts = app.store.artifacts(1, &fork.id).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].node, "a");
        assert_ne!(
            artifacts[0].id,
            app.store.artifacts(1, &r.id).unwrap()[0].id
        );
        std::fs::write(
            Path::new(&fork.workspace).join("finished-work.txt"),
            "new work",
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(Path::new(&r.workspace).join("finished-work.txt")).unwrap(),
            "Work already done"
        );
        let (same, created) = create(&app, 1, &r.id, w, None, &p.token).await.unwrap();
        assert!(!created);
        assert_eq!(same.id, fork.id);
        assert_eq!(app.store.runs(1).unwrap().len(), 2);
    }
    #[tokio::test]
    async fn gates_follow_only_the_successful_branch_and_cannot_skip_invalid_work() {
        let (_dir, app, mut r) = fixture().await;
        let gate:Node=serde_json::from_value(json!({"id":"gate","kind":"Gate","title":"Gate","agent":"agent","outcomes":["yes","no"]})).unwrap();
        let yes:Node=serde_json::from_value(json!({"id":"yes","kind":"Connection","title":"Yes","provider":"local","operation":"read_ticket","location":"README.md"})).unwrap();
        let mut no = yes.clone();
        no.id = "no".into();
        no.title = "No".into();
        r.workflow.nodes.extend([gate, yes, no]);
        r.workflow
            .edges
            .retain(|e| !(e.from == "a" && e.to == "review"));
        for (from, to, outcome) in [
            ("a", "gate", ""),
            ("gate", "yes", "yes"),
            ("gate", "no", "no"),
            ("yes", "review", ""),
            ("no", "review", ""),
        ] {
            r.workflow.edges.push(Edge {
                from: from.into(),
                to: to.into(),
                kind: "control".into(),
                outcome: outcome.into(),
            });
        }
        for node in ["gate", "yes"] {
            r.states.insert(node.into(), "completed".into());
            r.outputs.insert(
                node.into(),
                AgentResult {
                    status: "completed".into(),
                    decision: if node == "gate" { "yes" } else { "" }.into(),
                    ..Default::default()
                },
            );
        }
        r.states.insert("no".into(), "skipped".into());
        app.store.save_run(1, &r).unwrap();
        let mut w = r.workflow.clone();
        let mut b = w.node("yes").unwrap().clone();
        b.id = "b".into();
        b.title = "New B".into();
        w.nodes.push(b);
        w.edges.retain(|e| !(e.from == "yes" && e.to == "review"));
        for (from, to) in [("yes", "b"), ("b", "review")] {
            w.edges.push(Edge {
                from: from.into(),
                to: to.into(),
                kind: "control".into(),
                outcome: "".into(),
            });
        }
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        assert_eq!(p.restart.as_deref(), Some("b"));
        assert!(p.kept.iter().any(|n| n.id == "yes"));
        assert!(preview(&app, 1, &r.id, &w, Some("no")).is_err());
        assert!(preview(&app, 1, &r.id, &w, Some("review")).is_err());
        let p = preview(&app, 1, &r.id, &w, Some("gate")).unwrap();
        assert!(p.rerun.iter().any(|n| n.id == "yes"));
        assert!(p.kept.iter().any(|n| n.id == "a"));
    }
    #[tokio::test]
    async fn nested_workflow_changes_invalidate_prefix_and_preview_token() {
        let (_dir, app, mut r) = fixture().await;
        let mut grand = r.workflow.clone();
        grand.id = "grand".into();
        app.store.save_workflow(1, &grand).unwrap();
        let mut child = r.workflow.clone();
        child.id = "child".into();
        child.nodes.push(
            serde_json::from_value(
                json!({"id":"nested","kind":"Workflow","title":"Nested","workflow_id":"grand"}),
            )
            .unwrap(),
        );
        child.edges.retain(|e| !(e.from == "a" && e.to == "review"));
        for (from, to) in [("a", "nested"), ("nested", "review")] {
            child.edges.push(Edge {
                from: from.into(),
                to: to.into(),
                kind: "control".into(),
                outcome: "".into(),
            });
        }
        app.store.save_workflow(1, &child).unwrap();
        let mut grand_run = r.clone();
        grand_run.id = id();
        grand_run.workflow = app.store.workflow(1, "grand").unwrap();
        app.store.save_run(1, &grand_run).unwrap();
        let mut child_run = r.clone();
        child_run.id = id();
        child_run.workflow = app.store.workflow(1, "child").unwrap();
        child_run.states.insert("nested".into(), "completed".into());
        child_run.children.push(ChildRun {
            run: grand_run.id,
            node: "nested".into(),
            attempt: 1,
        });
        app.store.save_run(1, &child_run).unwrap();
        r.workflow.nodes.push(
            serde_json::from_value(
                json!({"id":"call","kind":"Workflow","title":"Call child","workflow_id":"child"}),
            )
            .unwrap(),
        );
        r.workflow
            .edges
            .retain(|e| !(e.from == "a" && e.to == "review"));
        for (from, to) in [("a", "call"), ("call", "review")] {
            r.workflow.edges.push(Edge {
                from: from.into(),
                to: to.into(),
                kind: "control".into(),
                outcome: "".into(),
            });
        }
        r.states.insert("call".into(), "completed".into());
        r.outputs.insert(
            "call".into(),
            AgentResult {
                status: "completed".into(),
                ..Default::default()
            },
        );
        r.children.push(ChildRun {
            run: child_run.id,
            node: "call".into(),
            attempt: 1,
        });
        app.store.save_run(1, &r).unwrap();
        let mut w = r.workflow.clone();
        w.nodes.push(serde_json::from_value(json!({"id":"b","kind":"Connection","title":"New B","provider":"local","operation":"read_ticket","location":"README.md"})).unwrap());
        w.edges.push(Edge {
            from: "review".into(),
            to: "b".into(),
            kind: "control".into(),
            outcome: "".into(),
        });
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        assert!(p.kept.iter().any(|n| n.id == "call"));
        assert_eq!(p.restart.as_deref(), Some("b"));
        grand.nodes[2].prompt = "Changed nested task".into();
        app.store.save_workflow(1, &grand).unwrap();
        let changed = preview(&app, 1, &r.id, &w, None).unwrap();
        assert_eq!(changed.restart.as_deref(), Some("call"));
        assert!(changed.rerun.iter().any(|n| n.id == "call"));
        assert_ne!(p.token, changed.token);
        grand.nodes[2].prompt = "Changed again while already invalid".into();
        app.store.save_workflow(1, &grand).unwrap();
        let again = preview(&app, 1, &r.id, &w, None).unwrap();
        assert_eq!(again.rerun.len(), changed.rerun.len());
        assert_ne!(again.token, changed.token);
        assert!(
            create(&app, 1, &r.id, w, None, &changed.token)
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn ambiguous_control_routes_are_rejected_before_preview() {
        let (_dir, app, r) = fixture().await;
        let mut w = insert(&r);
        w.edges.push(Edge {
            from: "a".into(),
            to: "review".into(),
            kind: "control".into(),
            outcome: "".into(),
        });
        assert!(
            preview(&app, 1, &r.id, &w, None)
                .unwrap_err()
                .to_string()
                .contains("Più rami per lo stesso esito")
        );
    }
    #[tokio::test]
    async fn direct_sources_become_independent_copies() {
        let (_dir, app, mut r) = fixture().await;
        let root = app.sources.join("original");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("project.txt"), "original source work").unwrap();
        r.workspace = std::fs::canonicalize(&root)
            .unwrap()
            .to_string_lossy()
            .into();
        r.workflow.nodes[0].source_type = "folder".into();
        r.workflow.nodes[0].source_mode = "direct".into();
        r.workflow.nodes[0].location = "original".into();
        r.context
            .insert("direct-source:source".into(), r.workspace.clone());
        r.context.insert("direct-primary".into(), "source".into());
        app.store.save_run(1, &r).unwrap();
        let w = insert(&r);
        let p = preview(&app, 1, &r.id, &w, None).unwrap();
        let (fork, _) = create(&app, 1, &r.id, w, None, &p.token).await.unwrap();
        assert_eq!(fork.workflow.nodes[0].source_mode, "copy");
        assert!(crate::source_access::paths(&fork.context).is_empty());
        crate::source_access::validate_bindings(&app, &fork).unwrap();
        assert_eq!(
            std::fs::read_to_string(Path::new(&fork.workspace).join("project.txt")).unwrap(),
            "original source work"
        );
    }
}
