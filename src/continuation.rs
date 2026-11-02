//! Explicitly resume a stopped run from a verified control-flow checkpoint.
use crate::{App, agents, engine, model::*};
use anyhow::{Result, ensure};
use std::collections::HashSet;

fn predecessor(r: &Run, node: &str) -> Option<Edge> {
    r.workflow
        .edges
        .iter()
        .find(|edge| {
            if edge.kind != "control"
                || edge.to != node
                || r.states.get(&edge.from).is_none_or(|s| s != "completed")
            {
                return false;
            }
            let Ok(previous) = r.workflow.node(&edge.from) else {
                return false;
            };
            let Some(output) = r
                .outputs
                .get(&edge.from)
                .filter(|o| o.status == "completed")
            else {
                return false;
            };
            let outcome = if previous.kind == "Gate" {
                output.decision.as_str()
            } else {
                ""
            };
            r.workflow
                .next(&edge.from, outcome)
                .is_some_and(|selected| selected.kind == "control" && selected.to == node)
        })
        .cloned()
}
fn restart_point(r: &Run, node: &str) -> bool {
    node == r.workflow.entry
        || r.workflow
            .nodes
            .iter()
            .find(|n| n.kind == "Source")
            .is_some_and(|n| n.id == node)
}
fn available(app: &App, uid: i64, r: &Run, busy: &HashSet<String>, restarting: bool) -> Result<()> {
    ensure!(
        !busy.contains(&r.id),
        "Metti la run in pausa prima di scegliere un nodo"
    );
    ensure!(
        (["paused", "failed", "interrupted", "completed"].contains(&r.status.as_str())
            || restarting && ["waiting_review", "waiting_user"].contains(&r.status.as_str()))
            && (restarting || r.pending.is_none()),
        "Risolvi le richieste in attesa prima di continuare da un nodo"
    );
    let mut children: Vec<_> = r.children.iter().map(|call| call.run.clone()).collect();
    let mut seen = HashSet::new();
    while let Some(id) = children.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let child = app.store.run(uid, &id)?;
        ensure!(
            ["completed", "failed", "interrupted"].contains(&child.status.as_str())
                && child.pending.is_none(),
            "Risolvi le attività e le richieste del workflow figlio prima di scegliere un nodo"
        );
        children.extend(child.children.iter().map(|call| call.run.clone()));
    }
    crate::subworkflows::can_launch_reserved(app, uid, r, busy)
}
pub fn candidates(app: &App, uid: i64, r: &Run) -> Vec<String> {
    let busy = app.busy.lock().unwrap();
    let can_restart = available(app, uid, r, &busy, true).is_ok();
    let can_continue = available(app, uid, r, &busy, false).is_ok();
    r.workflow
        .nodes
        .iter()
        .filter(|n| {
            if restart_point(r, &n.id) {
                can_restart
            } else {
                can_continue && predecessor(r, &n.id).is_some()
            }
        })
        .map(|n| n.id.clone())
        .collect()
}
pub fn prepare(app: &App, uid: i64, rid: &str, node: &str, acknowledge: bool) -> Result<Run> {
    let busy = app.busy.lock().unwrap();
    let mut r = app.store.run(uid, rid)?;
    let restarting = restart_point(&r, node);
    available(app, uid, &r, &busy, restarting)?;
    let target = if restarting {
        r.workflow.entry.clone()
    } else {
        node.into()
    };
    let node = target.as_str();
    let incoming = if restarting {
        None
    } else {
        predecessor(&r, node)
    };
    ensure!(
        restarting || incoming.is_some(),
        "Il nodo precedente non è completato con successo sul percorso selezionato"
    );
    ensure!(
        acknowledge,
        "Conferma la ripresa: i passi da questo nodo saranno rieseguiti sui file correnti"
    );
    let mut affected = HashSet::new();
    let mut pending = vec![node.to_string()];
    while let Some(id) = pending.pop() {
        if !affected.insert(id.clone()) {
            continue;
        }
        pending.extend(
            r.workflow
                .edges
                .iter()
                .filter(|e| e.from == id && e.kind == "control")
                .map(|e| e.to.clone()),
        );
    }
    // Preserve attempts, rework/supervisor budgets, sessions, child history and artifacts.
    for id in &affected {
        r.states.insert(id.clone(), "queued".into());
        r.outputs.remove(id);
        for prefix in [
            "human-approved",
            "answer",
            "feedback",
            "test-proof",
            "effect",
            "effect-preview",
        ] {
            r.context.remove(&format!("{prefix}:{id}"));
        }
        if let Ok(n) = r.workflow.node(id)
            && n.kind == "Memory"
        {
            r.context.remove(&format!("handoff:{}", n.target));
        }
    }
    r.last_task = None;
    let mut cursor = incoming.as_ref().map(|edge| edge.from.clone());
    let mut seen = HashSet::new();
    while let Some(id) = cursor {
        if !seen.insert(id.clone()) {
            break;
        }
        if r.workflow.node(&id)?.kind == "Task" {
            r.last_task = Some(id);
            break;
        }
        cursor = predecessor(&r, &id).map(|edge| edge.from);
    }
    r.current = node.into();
    if let Some(edge) = &incoming
        && let Ok(previous) = r.workflow.node(&edge.from)
        && previous.kind == "Memory"
        && let Some(output) = r.outputs.get(&previous.id)
    {
        r.context.insert(
            format!("handoff:{}", previous.target),
            output.summary.clone(),
        );
    }
    r.last_node = incoming.as_ref().map(|edge| edge.from.clone());
    r.active_edge = incoming;
    r.pending = None;
    r.status = "paused".into();
    r.error = None;
    r.pause_requested = false;
    r.context.remove("review-feedback");
    engine::save(app, uid, &mut r)?;
    agents::emit(
        app,
        uid,
        rid,
        "continue",
        node,
        "Ripresa esplicita dal nodo: risultati e approvazioni a valle invalidati; file correnti e sessioni conservati",
    );
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn fixture() -> (tempfile::TempDir, App, Run) {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let w = crate::presets::catalog()
            .into_iter()
            .find(|template| template.id == "as-is-sdd")
            .expect("baseline template for checkpoint tests")
            .workflow;
        let mut r = engine::create(&app, 1, w, "test".into(), None)
            .await
            .unwrap();
        for n in &r.workflow.nodes {
            if !["Source", "Agent"].contains(&n.kind.as_str()) {
                r.states.insert(n.id.clone(), "completed".into());
                r.outputs.insert(
                    n.id.clone(),
                    AgentResult {
                        status: "completed".into(),
                        decision: "pass".into(),
                        ..Default::default()
                    },
                );
            }
        }
        r.status = "completed".into();
        app.store.save_run(1, &r).unwrap();
        (dir, app, r)
    }
    #[tokio::test]
    async fn continuation_preserves_checkpoint_and_invalidates_downstream_approvals() {
        let (_dir, app, mut r) = fixture().await;
        r.attempts.insert("step-1".into(), 2);
        r.reworks.insert("baseline-review".into(), 1);
        r.context.insert("supervisor:count".into(), "3".into());
        for prefix in [
            "human-approved",
            "effect",
            "effect-preview",
            "answer",
            "test-proof",
        ] {
            r.context.insert(format!("{prefix}:step-1"), "stale".into());
        }
        r.context
            .insert("human-approved:baseline-review".into(), "stale".into());
        app.store.save_run(1, &r).unwrap();
        assert!(candidates(&app, 1, &r).contains(&"step-1".into()));
        assert!(candidates(&app, 1, &r).contains(&"step-0".into()));
        assert!(prepare(&app, 1, &r.id, "step-1", false).is_err());
        assert!(prepare(&app, 2, &r.id, "step-1", true).is_err());
        let resumed = prepare(&app, 1, &r.id, "step-1", true).unwrap();
        assert_eq!(resumed.id, r.id);
        assert_eq!(resumed.workspace, r.workspace);
        assert_eq!(resumed.current, "step-1");
        assert_eq!(resumed.last_node.as_deref(), Some("step-0"));
        assert_eq!(resumed.last_task.as_deref(), Some("step-0"));
        assert_eq!(resumed.states["step-0"], "completed");
        assert!(resumed.outputs.contains_key("step-0"));
        for id in ["step-1", "step-2", "step-3", "baseline-review"] {
            assert_eq!(resumed.states[id], "queued");
            assert!(!resumed.outputs.contains_key(id));
        }
        assert!(!resumed.context.contains_key("effect:step-1"));
        assert!(
            !resumed
                .context
                .contains_key("human-approved:baseline-review")
        );
        assert_eq!(resumed.attempts["step-1"], 2);
        assert_eq!(resumed.reworks["baseline-review"], 1);
        assert_eq!(resumed.context["supervisor:count"], "3");
    }
    #[tokio::test]
    async fn first_node_restarts_without_predecessor_and_invalidates_pending_approval() {
        let (_dir, app, mut r) = fixture().await;
        r.status = "waiting_review".into();
        r.pending = Some(Pending {
            id: "approval".into(),
            proposal: None,
            kind: "review".into(),
            node: "baseline-review".into(),
            questions: vec![],
            resume_task: None,
        });
        app.store.save_run(1, &r).unwrap();
        let ids = candidates(&app, 1, &r);
        assert!(ids.contains(&"source".into()));
        assert!(ids.contains(&"step-0".into()));
        assert!(!ids.contains(&"step-1".into()));
        let resumed = prepare(&app, 1, &r.id, "source", true).unwrap();
        assert_eq!(resumed.current, "step-0");
        assert!(resumed.last_node.is_none());
        assert!(resumed.last_task.is_none());
        assert!(resumed.active_edge.is_none());
        assert!(resumed.pending.is_none());
        assert!(resumed.outputs.is_empty());
        assert_eq!(resumed.workspace, r.workspace);
        assert_eq!(resumed.id, r.id);
        r.states.remove("step-0");
        r.outputs.remove("step-0");
        r.pending = None;
        r.status = "paused".into();
        app.store.save_run(1, &r).unwrap();
        assert!(prepare(&app, 1, &r.id, "step-0", true).is_ok());
        assert!(prepare(&app, 1, &r.id, "step-1", true).is_err());
        app.busy.lock().unwrap().insert(r.id.clone());
        assert!(prepare(&app, 1, &r.id, "source", true).is_err());
    }
    #[tokio::test]
    async fn continuation_rejects_unsuccessful_predecessors_busy_runs_and_pending_reviews() {
        let (_dir, app, mut r) = fixture().await;
        app.busy.lock().unwrap().insert(r.id.clone());
        assert!(candidates(&app, 1, &r).is_empty());
        assert!(prepare(&app, 1, &r.id, "step-1", true).is_err());
        app.busy.lock().unwrap().clear();
        for status in ["failed", "running", "skipped", "queued"] {
            r.states.insert("step-0".into(), status.into());
            app.store.save_run(1, &r).unwrap();
            assert!(prepare(&app, 1, &r.id, "step-1", true).is_err());
        }
        r.states.insert("step-0".into(), "completed".into());
        r.status = "waiting_review".into();
        app.store.save_run(1, &r).unwrap();
        assert!(prepare(&app, 1, &r.id, "step-1", true).is_err());
        r.status = "paused".into();
        r.pending = Some(Pending {
            id: "pending".into(),
            proposal: None,
            kind: "review".into(),
            node: "baseline-review".into(),
            questions: vec![],
            resume_task: None,
        });
        app.store.save_run(1, &r).unwrap();
        assert!(prepare(&app, 1, &r.id, "step-1", true).is_err());
    }
    #[tokio::test]
    async fn continuation_only_follows_the_successful_gate_branch() {
        let mut w = crate::presets::all().remove(1);
        let gate = w.nodes.iter_mut().find(|n| n.kind == "Gate").unwrap();
        let gid = gate.id.clone();
        let selected = w.next(&gid, "spec_change").unwrap().to.clone();
        let (_dir, _app, mut r) = fixture().await;
        r.workflow = w;
        r.states.clear();
        r.outputs.clear();
        r.states.insert(gid.clone(), "completed".into());
        r.outputs.insert(
            gid.clone(),
            AgentResult {
                status: "completed".into(),
                decision: "spec_change".into(),
                ..Default::default()
            },
        );
        assert!(predecessor(&r, &selected).is_some());
        r.outputs.get_mut(&gid).unwrap().decision = "needs_info".into();
        assert!(predecessor(&r, &selected).is_none());
    }
}
