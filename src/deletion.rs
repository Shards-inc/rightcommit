use crate::{
    App,
    auth::{ApiResult, bad},
    model::*,
};
use anyhow::{Result, ensure};
use axum::{
    Json,
    extract::{Path, State},
};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::{Path as FsPath, PathBuf},
};
fn runs(app: &App, uid: i64, wid: &str) -> Result<Vec<Run>> {
    app.store.workflow(uid, wid)?;
    let all: Vec<Run> = {
        let db = app.store.db.lock().unwrap();
        let mut stmt = db.prepare("SELECT json FROM runs WHERE user_id=? ORDER BY id")?;
        stmt.query_map([uid], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect::<Result<_>>()?
    };
    let mut ids: HashSet<String> = all
        .iter()
        .filter(|r| r.workflow.id == wid)
        .map(|r| r.id.clone())
        .collect();
    loop {
        let before = ids.len();
        for r in &all {
            if r.parent.as_ref().is_some_and(|p| ids.contains(&p.run)) {
                ids.insert(r.id.clone());
            }
        }
        if ids.len() == before {
            break;
        }
    }
    ensure!(
        !all.iter().any(
            |r| ids.contains(&r.id) && r.parent.as_ref().is_some_and(|p| !ids.contains(&p.run))
        ),
        "Questo workflow ha esecuzioni richiamate da un altro workflow. Elimina prima il workflow principale per conservare la gerarchia e il workspace."
    );
    Ok(all.into_iter().filter(|r| ids.contains(&r.id)).collect())
}
fn active(app: &App, runs: &[Run]) -> bool {
    let busy = app.busy.lock().unwrap();
    runs.iter().any(|r| {
        busy.contains(&r.id)
            || ["starting", "running", "waiting_external", "waiting_child"]
                .contains(&r.status.as_str())
    })
}
pub async fn preview(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let _guard = app.lifecycle.read().await;
    let runs = runs(&app, u.id, &id)?;
    let chats = crate::manager::cleanup_records(
        &app,
        u.id,
        &id,
        &runs.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
    )?;
    let mut artifacts = 0;
    for r in &runs {
        artifacts += app.store.artifacts(u.id, &r.id)?.len()
    }
    Ok(Json(
        json!({"run_ids":runs.iter().map(|r|r.id.clone()).collect::<Vec<_>>(),"runs":runs.len(),"artifacts":artifacts,"active":active(&app,&runs)||chats.iter().any(|c|c.busy),"manager_chats":chats.len(),"schedules":crate::schedules::workflow_count(&app,u.id,&id)?}),
    ))
}
#[derive(Deserialize)]
pub struct Confirmation {
    #[serde(default)]
    acknowledge: bool,
    run_ids: Vec<String>,
}
// Resolve existing ancestors too: a missing sources root or a symlink in a
// parent directory must not turn into permission to delete source material.
fn resolved_location(path: &FsPath) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut existing = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(
                    existing
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("Percorso di pulizia non valido"))?
                        .to_os_string(),
                );
                existing = existing
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("Percorso di pulizia non valido"))?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut resolved = std::fs::canonicalize(existing)?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn protected_sources(app: &App, path: &FsPath) -> Result<bool> {
    let sources = resolved_location(&app.sources)?;
    let candidate = resolved_location(path)?;
    // Protect descendants AND ancestors: removing a parent would remove sources.
    Ok(candidate.starts_with(&sources) || sources.starts_with(&candidate))
}

fn native_files(
    app: &App,
    dir: &FsPath,
    ids: &HashSet<String>,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    if !dir.exists() || dir.is_symlink() || protected_sources(app, dir)? {
        return Ok(());
    }
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_symlink() || protected_sources(app, &p)? {
            continue;
        }
        let name = p.file_name().unwrap().to_string_lossy();
        if ids.iter().any(|id| name.contains(id)) {
            out.push(p)
        } else if p.is_dir() {
            native_files(app, &p, ids, out)?
        }
    }
    Ok(())
}
pub(crate) fn perform(app: &App, uid: i64, wid: &str, expected: &[String]) -> Result<Value> {
    let runs = runs(app, uid, wid)?;
    ensure!(
        !active(app, &runs),
        "Metti in pausa le esecuzioni attive e attendi la fine del turno prima di eliminare il workflow."
    );
    let ids: Vec<_> = runs.iter().map(|r| r.id.clone()).collect();
    let mut expected = expected.to_vec();
    expected.sort();
    ensure!(
        ids == expected,
        "Le esecuzioni sono cambiate. Riapri la conferma di eliminazione."
    );
    let chats = crate::manager::cleanup_records(app, uid, wid, &ids)?;
    ensure!(
        !chats.iter().any(|c| c.busy),
        "Attendi la risposta del project manager prima di eliminare il workflow"
    );
    let deleted_chats: Vec<_> = chats.iter().map(|c| c.id.clone()).collect();
    let home = app.data.join("users").join(uid.to_string());
    let mut paths = Vec::new();
    for run in &runs {
        ensure!(
            uuid::Uuid::parse_str(&run.id).is_ok(),
            "ID esecuzione non valido"
        );
        // Only run-owned directories may be moved to trash. Direct /sources
        // bindings belong to the shared catalog, including for child runs.
        let lineage = crate::subworkflows::ancestors(app, uid, run)?;
        let root = lineage.last().unwrap_or(run);
        let direct_primary = root.context.contains_key("direct-primary");
        let p = if direct_primary {
            ensure!(
                uuid::Uuid::parse_str(&root.id).is_ok(),
                "ID esecuzione non valido"
            );
            home.join("workspaces").join(crate::subworkflows::workspace_owner(root)?)
        } else {
            crate::subworkflows::workspace(app, uid, run)?
        };
        if p.exists() {
            if protected_sources(app, &p)? {
                continue;
            }
            ensure!(
                !p.is_symlink() && !home.is_symlink() && !home.join("workspaces").is_symlink(),
                "Workspace non valido"
            );
            ensure!(
                direct_primary
                    || std::fs::canonicalize(&p)? == std::fs::canonicalize(&run.workspace)?,
                "Workspace non valido"
            );
            if !paths.contains(&p) {
                paths.push(p)
            }
        }
    }
    for chat in &chats {
        if chat.workspace.exists() {
            ensure!(
                !chat
                    .workspace
                    .ancestors()
                    .take_while(|p| p.starts_with(&app.data))
                    .any(|p| p.is_symlink()),
                "Spazio manager non valido"
            );
            paths.push(chat.workspace.clone());
        }
    }
    let mut native: HashSet<_> = runs
        .iter()
        .flat_map(|r| r.sessions.values().map(|s| s.native_id.clone()))
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        .chain(chats.iter().flat_map(|c| {
            c.sessions
                .iter()
                .filter(|id| uuid::Uuid::parse_str(id).is_ok())
                .cloned()
        }))
        .collect();
    for id in crate::manager::retained_sessions(app, uid, &deleted_chats)? {
        native.remove(&id);
    }
    // Retain any native session still referenced by a different workflow.
    {
        let db = app.store.db.lock().unwrap();
        let mut q = db.prepare("SELECT json FROM runs WHERE user_id=?")?;
        for row in q.query_map(params![uid], |r| r.get::<_, String>(0))? {
            let r: Run = serde_json::from_str(&row?)?;
            if ids.contains(&r.id) {
                continue;
            }
            for s in r.sessions.values() {
                native.remove(&s.native_id);
            }
        }
    }
    for folder in [".codex/sessions", ".claude/projects", ".pi/agent/sessions"] {
        native_files(app, &home.join(folder), &native, &mut paths)?
    }
    // This final guard covers every cleanup candidate, including native-session
    // paths, independently of Source mode, workflow lineage, or stored metadata.
    let mut cleanup_paths = Vec::new();
    for path in paths {
        if !protected_sources(app, &path)? {
            cleanup_paths.push(path);
        }
    }
    // A historical workflow must never remove a workspace still referenced by
    // another run, including secondary Sources and the auxiliary data folder.
    let retained: Vec<Run> = {
        let db = app.store.db.lock().unwrap();
        let mut q = db.prepare("SELECT json FROM runs WHERE user_id=?")?;
        q.query_map([uid], |row|row.get::<_,String>(0))?
            .map(|row|Ok(serde_json::from_str(&row?)?)).collect::<Result<Vec<Run>>>()?
    };
    let mut retained_paths = Vec::new();
    for run in retained.iter().filter(|r|!ids.contains(&r.id)) {
        let lineage = crate::subworkflows::ancestors(app, uid, run)?;
        let root = lineage.last().unwrap_or(run);
        retained_paths.push(resolved_location(FsPath::new(&root.workspace))?);
        retained_paths.push(resolved_location(&home.join("workspaces").join(crate::subworkflows::workspace_owner(root)?))?);
        for (index,node) in root.workflow.nodes.iter().filter(|n|n.kind=="Source").enumerate() {
            let p = PathBuf::from(crate::source_access::source_path(root,index,node));
            retained_paths.push(resolved_location(&if p.is_absolute(){p}else{FsPath::new(&root.workspace).join(p)})?);
        }
    }
    let paths: Vec<_> = cleanup_paths.into_iter().filter(|p| {
        let p = resolved_location(p);
        p.as_ref().is_ok_and(|p|!retained_paths.iter().any(|r|p.starts_with(r)||r.starts_with(p)))
    }).collect();
    let trash = app.data.join("system/deletions").join(crate::model::id());
    if !paths.is_empty() {
        ensure!(
            !protected_sources(app, &trash)?,
            "L'area di pulizia si sovrappone a sources: nessun file eliminato"
        );
        std::fs::create_dir_all(&trash)?;
        crate::store::private(&trash, true)?;
    }
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let result = (|| -> Result<()> {
        for (i, p) in paths.iter().enumerate() {
            ensure!(
                !protected_sources(app, p)?,
                "Percorso sources protetto: nessun file eliminato"
            );
            let dest = trash.join(i.to_string());
            std::fs::rename(p, &dest)?;
            moved.push((p.clone(), dest))
        }
        let mut db = app.store.db.lock().unwrap();
        let tx = db.transaction()?;
        for rid in &ids {
            for table in ["artifacts", "artifact_observations", "artifact_baselines", "artifact_recoveries", "decisions", "events"] {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE user_id=? AND run=?"),
                    params![uid, rid],
                )?;
            }
            tx.execute(
                "DELETE FROM schedule_occurrences WHERE user_id=? AND run_id=?",
                params![uid, rid],
            )?;
            tx.execute(
                "DELETE FROM runs WHERE user_id=? AND id=?",
                params![uid, rid],
            )?;
        }
        for id in &deleted_chats {
            tx.execute(
                "DELETE FROM events WHERE user_id=? AND run=?",
                params![uid, id],
            )?;
            tx.execute(
                "DELETE FROM manager_chats WHERE user_id=? AND id=?",
                params![uid, id],
            )?;
        }
        tx.execute("DELETE FROM schedule_occurrences WHERE user_id=? AND schedule_id IN (SELECT id FROM schedules WHERE user_id=? AND workflow_id=?)", params![uid,uid,wid])?;
        tx.execute("DELETE FROM schedules WHERE user_id=? AND workflow_id=?", params![uid,wid])?;
        ensure!(
            tx.execute(
                "DELETE FROM workflows WHERE user_id=? AND id=?",
                params![uid, wid]
            )? == 1,
            "Workflow non trovato"
        );
        tx.execute(
            "DELETE FROM a2a_configs WHERE user_id=? AND kind='publication' AND id=?",
            params![uid, wid],
        )?;
        tx.execute("DELETE FROM a2a_replies WHERE task IN (SELECT id FROM a2a_tasks WHERE user_id=? AND workflow=?)", params![uid, wid])?;
        tx.execute(
            "DELETE FROM a2a_tasks WHERE user_id=? AND workflow=?",
            params![uid, wid],
        )?;
        tx.commit()?;
        Ok(())
    })();
    if let Err(e) = result {
        for (original, staged) in moved.iter().rev() {
            std::fs::rename(staged, original)?;
        }
        if !paths.is_empty() && !protected_sources(app, &trash)? {
            let _ = std::fs::remove_dir(&trash);
        }
        return Err(e);
    }
    let artifact_cleanup_pending = crate::artifacts::cleanup(app, uid).is_err();
    let cleanup_pending = artifact_cleanup_pending || !paths.is_empty()
        && (|| -> Result<()> {
            ensure!(!protected_sources(app, &trash)?, "Area sources protetta");
            Ok(std::fs::remove_dir_all(&trash)?)
        })()
        .is_err();
    Ok(json!({"ok":true,"deleted_runs":ids.len(),"cleanup_pending":cleanup_pending}))
}
pub async fn delete(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(c): Json<Confirmation>,
) -> ApiResult<Json<Value>> {
    if !c.acknowledge {
        return Err(bad(
            "Conferma l'eliminazione del workflow e delle esecuzioni.",
        ));
    }
    let lifecycle = app.lifecycle.clone();
    let _guard = lifecycle.write().await;
    let mcp = app.mcp.clone();
    let uid = u.id;
    let deleted_runs = c.run_ids.clone();
    let result = tokio::task::spawn_blocking(move || perform(&app, u.id, &id, &c.run_ids))
        .await
        .map_err(|_| bad("Eliminazione interrotta"))??;
    for rid in deleted_runs {
        mcp.release_run(uid, &rid).await;
    }
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn deletes_owned_workflow_cascade_and_rolls_back_failures() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let workflow = crate::presets::all().remove(0);
        app.store.save_workflow(1, &workflow).unwrap();
        app.store.save_workflow(2, &workflow).unwrap();
        let mut run = crate::engine::create(&app, 1, workflow.clone(), "test".into(), None)
            .await
            .unwrap();
        let second = crate::engine::create(&app, 1, workflow.clone(), "second".into(), None)
            .await
            .unwrap();
        let other = crate::engine::create(&app, 2, workflow.clone(), "other user".into(), None)
            .await
            .unwrap();
        let sid = crate::model::id();
        run.sessions.insert(
            "author".into(),
            NativeSession {
                provider: String::new(), base_url: String::new(),
                native_id: sid.clone(),
                runtime: "pi".into(),
                turns: 1,
                model: String::new(),
            },
        );
        app.store.save_run(1, &run).unwrap();
        let session = app
            .data
            .join(format!("users/1/.pi/agent/sessions/test/{sid}.jsonl"));
        std::fs::create_dir_all(session.parent().unwrap()).unwrap();
        std::fs::write(&session, "test session").unwrap();
        crate::agents::snapshot(&app, 1, &run, "task", "author", "generated report").unwrap();
        app.store
            .add_decision(
                1,
                &Decision {
                    actor: "human".into(),
                    supervisor: None,
                    id: crate::model::id(),
                    run: run.id.clone(),
                    node: "review".into(),
                    decision: "test".into(),
                    answer: "isolated fixture".into(),
                    user: "test".into(),
                    created: now(),
                    artifacts: vec![],
                },
            )
            .unwrap();
        let keep = crate::presets::all().remove(1);
        app.store.save_workflow(1, &keep).unwrap();
        let mut ids = vec![run.id.clone(), second.id.clone()];
        ids.sort();
        assert!(perform(&app, 3, &workflow.id, &ids).is_err());
        assert!(perform(&app, 1, &workflow.id, &[]).is_err());
        app.busy.lock().unwrap().insert(run.id.clone());
        assert!(perform(&app, 1, &workflow.id, &ids).is_err());
        app.busy.lock().unwrap().remove(&run.id);
        app.store.db.lock().unwrap().execute_batch("CREATE TRIGGER refuse_deletion BEFORE DELETE ON workflows BEGIN SELECT RAISE(ABORT,'rollback probe'); END;").unwrap();
        assert!(perform(&app, 1, &workflow.id, &ids).is_err());
        assert!(FsPath::new(&run.workspace).exists());
        assert!(session.exists());
        assert!(!app.store.artifacts(1, &run.id).unwrap().is_empty());
        app.store
            .db
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER refuse_deletion")
            .unwrap();
        let result = perform(&app, 1, &workflow.id, &ids).unwrap();
        assert_eq!(result["deleted_runs"], 2);
        assert_eq!(result["cleanup_pending"], false);
        assert!(app.store.workflow(1, &workflow.id).is_err());
        assert!(app.store.run(1, &run.id).is_err());
        assert!(!FsPath::new(&run.workspace).exists());
        assert!(!FsPath::new(&second.workspace).exists());
        assert!(!session.exists());
        for table in ["artifacts", "decisions", "events", "runs"] {
            let count: i64 = app
                .store
                .db
                .lock()
                .unwrap()
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE user_id=1"),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }
        assert!(app.store.workflow(1, &keep.id).is_ok());
        assert!(app.store.workflow(2, &workflow.id).is_ok());
        assert!(app.store.run(2, &other.id).is_ok());
        assert!(FsPath::new(&other.workspace).exists());
    }
    fn inventory(root: &FsPath) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(root: &FsPath, path: &FsPath, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            for e in std::fs::read_dir(path).unwrap() {
                let path = e.unwrap().path();
                let bytes = if path.is_dir() {
                    Vec::new()
                } else {
                    std::fs::read(&path).unwrap()
                };
                out.push((path.strip_prefix(root).unwrap().to_path_buf(), bytes));
                if path.is_dir() {
                    walk(root, &path, out)
                }
            }
        }
        let mut result = Vec::new();
        walk(root, root, &mut result);
        result.sort();
        result
    }

    #[tokio::test]
    async fn deleting_source_copy_preserves_every_original_and_aliased_native_session() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let original = app.sources.join("material");
        std::fs::create_dir_all(original.join("nested/empty")).unwrap();
        std::fs::write(original.join("nested/notes.txt"), "original bytes").unwrap();
        let mut w = crate::presets::all().remove(0);
        let n = w.nodes.iter_mut().find(|n| n.kind == "Source").unwrap();
        n.source_type = "folder".into();
        n.source_mode = "copy".into();
        n.location = "material".into();
        app.store.save_workflow(1, &w).unwrap();
        let mut run = crate::engine::create(&app, 1, w.clone(), "test".into(), None)
            .await
            .unwrap();
        std::fs::write(
            FsPath::new(&run.workspace).join("private-result.txt"),
            "generated",
        )
        .unwrap();
        let sid = crate::model::id();
        let sessions = original.join(".pi/agent/sessions/catalog");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(
            sessions.join(format!("{sid}.jsonl")),
            "source session must survive",
        )
        .unwrap();
        let home_pi = app.data.join("users/1/.pi");
        std::fs::remove_dir_all(&home_pi).unwrap();
        std::os::unix::fs::symlink(original.join(".pi"), &home_pi).unwrap();
        run.sessions.insert(
            "author".into(),
            NativeSession {
                provider: String::new(), base_url: String::new(),
runtime: "pi".into(),
                native_id: sid,
                ..Default::default()
            },
        );
        app.store.save_run(1, &run).unwrap();
        let before = inventory(&original);
        perform(&app, 1, &w.id, std::slice::from_ref(&run.id)).unwrap();
        assert_eq!(inventory(&original), before);
        assert!(
            !FsPath::new(&run.workspace).exists(),
            "Private copies must still be removed"
        );
        assert!(app.store.run(1, &run.id).is_err());
    }

    #[tokio::test]
    async fn deletion_preserves_sources_inside_cleanup_parent_even_through_root_alias() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = crate::tests::app(dir.path());
        let w = crate::presets::all().remove(0);
        app.store.save_workflow(1, &w).unwrap();
        let run = crate::engine::create(&app, 1, w.clone(), "test".into(), None)
            .await
            .unwrap();
        let original = FsPath::new(&run.workspace).join("catalog");
        std::fs::create_dir_all(original.join("empty")).unwrap();
        std::fs::write(original.join("keep.bin"), [0, 1, 2, 255]).unwrap();
        app.sources = dir.path().join("source-alias");
        std::os::unix::fs::symlink(&original, &app.sources).unwrap();
        let before = inventory(&original);
        perform(&app, 1, &w.id, std::slice::from_ref(&run.id)).unwrap();
        assert_eq!(inventory(&original), before);
        assert!(
            FsPath::new(&run.workspace).exists(),
            "An ancestor containing sources cannot be removed"
        );
        assert!(app.store.run(1, &run.id).is_err());
    }

    #[test]
    fn cleanup_guard_protects_missing_source_locations_and_rejects_broken_aliases() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = crate::tests::app(dir.path());
        assert!(protected_sources(&app, &app.sources.join("missing/file")).unwrap());
        assert!(protected_sources(&app, dir.path()).unwrap());
        assert!(!protected_sources(&app, &dir.path().join("unrelated")).unwrap());
        app.sources = dir.path().join("broken");
        std::os::unix::fs::symlink(dir.path().join("missing"), &app.sources).unwrap();
        assert!(protected_sources(&app, &dir.path().join("unrelated")).is_err());
    }
}
