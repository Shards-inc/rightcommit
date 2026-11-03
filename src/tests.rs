use crate::{
    App, agents, engine,
    model::*,
    store::{Store, hash},
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
pub(crate) fn app(dir: &Path) -> App {
    let (bus, _) = tokio::sync::broadcast::channel(16);
    App {
        store: Store::open(dir).unwrap(),
        mcp: Arc::default(),
        rag: Arc::default(),
        data: dir.to_path_buf(),
        sources: dir.join("sources"),
        assets: Path::new(env!("CARGO_MANIFEST_DIR")).into(),
        busy: Arc::new(Mutex::new(Default::default())),
        workers: Arc::default(),
        lifecycle: Arc::default(),
        slots: Arc::default(),
        bus,
        isolate: false,
        secure_cookie: false,
        host_import: false,
    }
}
#[test]
fn both_presets_are_executable_graphs() {
    for w in crate::presets::all() {
        engine::validate(&w).unwrap();
    }
}
#[test]
fn memory_rejects_wrong_consumer_and_missing_source() {
    let mut w = crate::presets::all().remove(1);
    w.nodes
        .iter_mut()
        .find(|n| n.id == "handoff")
        .unwrap()
        .target = "triage-agent".into();
    assert!(
        engine::validate(&w)
            .unwrap_err()
            .to_string()
            .contains("Memory")
    );
    let mut w = crate::presets::all().remove(1);
    w.nodes
        .iter_mut()
        .find(|n| n.id == "handoff")
        .unwrap()
        .agent = "implement".into();
    assert!(
        engine::validate(&w)
            .unwrap_err()
            .to_string()
            .contains("sorgente")
    );
}
#[test]
fn rejects_cycles_disconnected_nodes_and_unrouted_gates() {
    let mut w = crate::presets::all().remove(0);
    w.edges.push(Edge {
        from: "analyze".into(),
        to: "spec".into(),
        kind: "control".into(),
        outcome: "".into(),
    });
    assert!(
        engine::validate(&w)
            .unwrap_err()
            .to_string()
            .contains("Ciclo")
    );
    let mut w = crate::presets::all().remove(0);
    w.edges.retain(|e| e.to != "plan");
    assert!(engine::validate(&w).is_err());
    let mut w = crate::presets::all().remove(0);
    w.nodes
        .iter_mut()
        .find(|n| n.id == "ready")
        .unwrap()
        .outcomes
        .push("invented".into());
    assert!(
        engine::validate(&w)
            .unwrap_err()
            .to_string()
            .contains("ramo")
    );
}
#[test]
fn encrypted_settings_persist_and_reject_tampering() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path()).unwrap();
    let mut cfg = Settings::default();
    cfg.agents.get_mut("codex").unwrap().api_key = "unit-test-private-secret".into();
    s.save_settings(1, &cfg).unwrap();
    assert_eq!(s.settings(2).unwrap().agents["codex"].api_key, "");
    let raw: Vec<u8> =
        s.db.lock()
            .unwrap()
            .query_row("SELECT blob FROM settings WHERE user_id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert!(!String::from_utf8_lossy(&raw).contains("private-secret"));
    let mut corrupt = raw.clone();
    corrupt[20] ^= 1;
    assert!(s.decrypt(&corrupt).is_err());
    drop(s);
    let s = Store::open(d.path()).unwrap();
    assert_eq!(
        s.settings(1).unwrap().agents["codex"].api_key,
        "unit-test-private-secret"
    );
}
#[test]
fn immutable_artifacts_and_ownership() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path()).unwrap();
    let a = Artifact {
        file: None,
        id: id(),
        run: "run".into(),
        node: "task".into(),
        path: "spec.md".into(),
        version: 1,
        hash: hash("old"),
        content: "old".into(),
        session: "native-id".into(),
        turn: 1,
        created: now(),
    };
    s.add_artifact(1, &a).unwrap();
    assert!(s.artifact(2, &a.id).is_err());
    assert!(s.add_artifact(1, &a).is_err());
    assert_eq!(s.artifact(1, &a.id).unwrap().content, "old");
}
#[test]
fn structured_results_fail_closed() {
    let r=agents::parse_result("Report\n<nodo_result>{\"status\":\"needs_input\",\"summary\":\"question\",\"questions\":[\"Which SLA?\"]}</nodo_result>").unwrap();
    assert_eq!(r.details, "Report");
    assert!(r.report().contains("Report"));
    assert_eq!(r.questions.len(), 1);
    assert!(agents::parse_result("All tests passed, trust me").is_err());
    assert!(agents::parse_result("{\"status\":\"needs_input\",\"questions\":[]}").is_err());
    assert!(agents::parse_result("{\"status\":\"approved_by_human\"}").is_err());
}
#[test]
fn truncated_ci_cannot_approve_delivery() {
    let good = serde_json::json!({"total_count":1,"check_runs":[{"status":"completed","conclusion":"success"}]});
    crate::connectors::validate_ci(&good).unwrap();
    let mut truncated = good.clone();
    truncated["total_count"] = serde_json::json!(2);
    assert!(crate::connectors::validate_ci(&truncated).is_err());
    let mut failed = good;
    failed["check_runs"][0]["conclusion"] = serde_json::json!("failure");
    assert!(crate::connectors::validate_ci(&failed).is_err());
}
#[test]
fn markdown_keeps_task_status_and_removes_executable_html() {
    let html = crate::api::render_markdown(
        "# Tasks\n- [x] Completed\n- [ ] Waiting\n<script>alert(1)</script><img src=x onerror=alert(2)>\n",
    );
    assert!(html.contains("[x]") && html.contains("[ ]"));
    assert!(!html.contains("<script") && !html.contains("onerror"));
}
#[test]
fn convergence_requires_real_closed_checkboxes() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("specs/001-feature");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("tasks.md");
    std::fs::write(&p, "# Tasks\n- [x] T001 done\n- [ ] T002 remaining\n").unwrap();
    assert_eq!(engine::open_tasks(d.path(), "").unwrap(), 1);
    std::fs::write(&p, "# Tasks\n- [x] T001 done\n- [x] T002 done\n").unwrap();
    assert_eq!(engine::open_tasks(d.path(), "").unwrap(), 0);
    std::fs::write(&p, "Everything is done, trust this text").unwrap();
    assert!(engine::open_tasks(d.path(), "").is_err());
}
#[test]
fn real_installed_speckit_commands() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/helpdesk");
    assert_eq!(
        agents::resolve_command("specify", "codex", &root).unwrap(),
        "$speckit-specify"
    );
    assert_eq!(
        agents::resolve_command("/speckit.constitution", "claude", &root).unwrap(),
        "/speckit-constitution"
    );
    assert_eq!(
        agents::resolve_command("converge", "pi", &root).unwrap(),
        "/speckit.converge"
    );
    assert!(agents::resolve_command("../../secret", "codex", &root).is_err());
    assert!(agents::resolve_command("invented", "codex", &root).is_err());
}
#[test]
fn source_copy_refuses_symlinks() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("source")).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", d.path().join("source/link")).unwrap();
    assert!(agents::copy_tree(&d.path().join("source"), &d.path().join("dest")).is_err());
}

#[test]
fn source_copy_limit_is_inclusive_and_preflight_rejects_oversized_files() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("source");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    let file = std::fs::File::create(source.join("nested/material.bin")).unwrap();
    file.set_len(Settings::default().source_copy_limit_bytes().unwrap())
        .unwrap();
    assert_eq!(
        agents::source_copy_size(
            &source,
            Settings::default().source_copy_limit_bytes().unwrap()
        )
        .unwrap(),
        10 * 1024 * 1024
    );
    agents::copy_source_tree(
        &source,
        &d.path().join("allowed"),
        Settings::default().source_copy_limit_bytes().unwrap(),
    )
    .unwrap();
    assert_eq!(
        std::fs::metadata(d.path().join("allowed/nested/material.bin"))
            .unwrap()
            .len(),
        Settings::default().source_copy_limit_bytes().unwrap()
    );
    file.set_len(Settings::default().source_copy_limit_bytes().unwrap() + 1)
        .unwrap();
    let dest = d.path().join("rejected");
    let error = agents::copy_source_tree(
        &source,
        &dest,
        Settings::default().source_copy_limit_bytes().unwrap(),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("material.bin") && error.contains("10 MiB"));
    assert!(!dest.exists(), "Reject before creating an incomplete copy");
}

#[test]
fn snapshots_preserve_files_and_folders_larger_than_source_limit() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("remote-snapshot");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    let size = Settings::default().source_copy_limit_bytes().unwrap() + 1;
    std::fs::File::create(source.join("nested/large.bin"))
        .unwrap()
        .set_len(size)
        .unwrap();
    std::fs::write(source.join("nested/other.txt"), "Preserved").unwrap();
    let dest = d.path().join("workspace");
    agents::copy_tree(&source, &dest).unwrap();
    assert_eq!(
        std::fs::metadata(dest.join("nested/large.bin"))
            .unwrap()
            .len(),
        size
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("nested/other.txt")).unwrap(),
        "Preserved"
    );
}

#[tokio::test]
async fn incident_source_has_real_speckit_prerequisites_before_triage() {
    let d = tempfile::tempdir().unwrap();
    let a = app(d.path());
    let r = engine::create(&a, 1, crate::presets::all().remove(1), "test".into(), None)
        .await
        .unwrap();
    let out = agents::tool(
        &a,
        1,
        Path::new(&r.workspace),
        "bash",
        &[
            ".specify/scripts/bash/check-prerequisites.sh",
            "--json",
            "--require-tasks",
            "--include-tasks",
        ],
        30,
    )
    .await
    .unwrap();
    assert!(out.contains("001-ticket-dedup"));
    assert!(
        Path::new(&r.workspace)
            .join("specs/001-ticket-dedup/tasks.md")
            .is_file()
    );
}
#[tokio::test]
async fn restart_preserves_native_sessions_and_interrupts_uncertain_work() {
    let d = tempfile::tempdir().unwrap();
    let a = app(d.path());
    let mut r = engine::create(&a, 1, crate::presets::all().remove(0), "test".into(), None)
        .await
        .unwrap();
    r.status = "running".into();
    r.sessions.insert(
        "author".into(),
        NativeSession {
            provider: String::new(), base_url: String::new(),
runtime: "codex".into(),
            native_id: "stable-native-id".into(),
            turns: 3,
            model: "explicit-model".into(),
        },
    );
    r.states.insert("active".into(), "running".into());
    r.states.insert("done".into(), "completed".into());
    r.active_edge = Some(Edge {
        from: "done".into(),
        to: "active".into(),
        kind: "control".into(),
        outcome: String::new(),
    });
    r.updated = 1;
    a.store.save_run(1, &r).unwrap();
    a.store.interrupt_running().unwrap();
    let restored = a.store.run(1, &r.id).unwrap();
    assert_eq!(restored.status, "interrupted");
    assert_eq!(restored.sessions["author"].native_id, "stable-native-id");
    assert_eq!(restored.sessions["author"].turns, 3);
    assert_eq!(restored.states["active"], "interrupted");
    assert_eq!(restored.states["done"], "completed");
    assert!(restored.active_edge.is_none());
    assert!(restored.updated > 1);
    let events = a.store.events(1, &r.id, 0).unwrap();
    assert!(events.iter().any(|e| e.kind == "restart"));
    assert!(
        events
            .iter()
            .any(|e| e.kind == "state" && e.text == "interrupted")
    );
    a.store.interrupt_running().unwrap();
    assert_eq!(events.len(), a.store.events(1, &r.id, 0).unwrap().len());
    assert!(a.store.run(2, &r.id).is_err());
}
#[tokio::test]
async fn pause_request_is_not_lost_by_a_concurrent_session_save() {
    let d = tempfile::tempdir().unwrap();
    let a = app(d.path());
    let mut stale = engine::create(&a, 1, crate::presets::all().remove(0), "test".into(), None)
        .await
        .unwrap();
    stale.status = "running".into();
    a.store.save_run(1, &stale).unwrap();
    let mut fresh = stale.clone();
    fresh.pause_requested = true;
    a.store.save_run(1, &fresh).unwrap();
    a.store.save_run(1, &stale).unwrap();
    assert!(a.store.run(1, &stale.id).unwrap().pause_requested);
    stale.status = "paused".into();
    a.store.save_run(1, &stale).unwrap();
    assert!(!a.store.run(1, &stale.id).unwrap().pause_requested);
}
#[tokio::test]
async fn rejected_review_passes_findings_to_the_same_author_session() {
    let d = tempfile::tempdir().unwrap();
    let a = app(d.path());
    let mut r = engine::create(&a, 1, crate::presets::all().remove(0), "test".into(), None)
        .await
        .unwrap();
    r.current = "code-review".into();
    r.status = "waiting_review".into();
    r.pending = Some(Pending {
        id: crate::model::id(),
        proposal: None,
        kind: "review".into(),
        node: "code-review".into(),
        questions: vec![],
        resume_task: None,
    });
    r.outputs.insert(
        "code-review".into(),
        AgentResult {
            status: "completed".into(),
            summary: "Missing equal-priority regression".into(),
            decision: "rework".into(),
            ..Default::default()
        },
    );
    r.sessions.insert(
        "author".into(),
        NativeSession {
            provider: String::new(), base_url: String::new(),
runtime: "codex".into(),
            native_id: "same-native-session".into(),
            turns: 4,
            model: "explicit-model".into(),
        },
    );
    a.store.save_run(1, &r).unwrap();
    let r = engine::decide(
        &a,
        &User {
            id: 1,
            email: "qa@test.local".into(),
        },
        &r.id,
        "reject",
        "Add the missing regression",
    )
    .unwrap();
    assert_eq!(r.current, "implement");
    assert!(r.context["feedback:implement"].contains("equal-priority"));
    assert!(r.context["feedback:implement"].contains("Add the missing"));
    assert_eq!(r.sessions["author"].native_id, "same-native-session");
    assert_eq!(r.active_edge.unwrap().kind, "rework");
}
#[tokio::test]
async fn gate_answer_resumes_the_gate_without_replaying_the_author() {
    let d = tempfile::tempdir().unwrap();
    let a = app(d.path());
    let mut r = engine::create(
        &a,
        1,
        crate::presets::all().remove(0),
        "immutable original".into(),
        None,
    )
    .await
    .unwrap();
    r.current = "converged".into();
    r.status = "waiting_user".into();
    r.pending = Some(Pending {
        id: crate::model::id(),
        proposal: None,
        kind: "question".into(),
        node: "converged".into(),
        questions: vec!["Approve checklist?".into()],
        resume_task: Some("converge".into()),
    });
    r.sessions.insert(
        "author".into(),
        NativeSession {
            provider: String::new(), base_url: String::new(),
runtime: "codex".into(),
            native_id: "author-kept".into(),
            turns: 7,
            model: "model".into(),
        },
    );
    r.sessions.insert(
        "gate:converged".into(),
        NativeSession {
            provider: String::new(), base_url: String::new(),
runtime: "codex".into(),
            native_id: "gate-kept".into(),
            turns: 1,
            model: "model".into(),
        },
    );
    a.store.save_run(1, &r).unwrap();
    let r = engine::decide(
        &a,
        &User {
            id: 1,
            email: "reviewer@test.local".into(),
        },
        &r.id,
        "answer",
        "Approved for the fixture",
    )
    .unwrap();
    assert_eq!(r.current, "converged");
    assert_eq!(r.sessions["author"].turns, 7);
    assert_eq!(r.sessions["gate:converged"].native_id, "gate-kept");
    let receipt: serde_json::Value = serde_json::from_str(&r.context["answer:converged"]).unwrap();
    assert_eq!(receipt["answer"], "Approved for the fixture");
    assert_eq!(receipt["user"], "reviewer@test.local");
    assert!(!receipt["id"].as_str().unwrap().is_empty());
    let r = engine::annotate(
        &a,
        &User {
            id: 1,
            email: "reviewer@test.local".into(),
        },
        &r.id,
        "Scope clarified",
    )
    .unwrap();
    assert_eq!(r.input, "immutable original");
    assert_eq!(r.sessions["author"].native_id, "author-kept");
    assert!(
        engine::annotate(
            &a,
            &User {
                id: 2,
                email: "other@test.local".into()
            },
            &r.id,
            "Unauthorized"
        )
        .is_err()
    );
}
/// A pi-extensions folder: Pi packages (by `pi` key or `pi-package` keyword), a plain library
/// and a declared dependency that is not installed.
fn pi_extensions_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let write = |rel: &str, json: serde_json::Value| {
        let dir = d.path().join(rel);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), json.to_string()).unwrap();
    };
    write(
        "",
        serde_json::json!({"pi":{"extensions":["./web-fetch.ts"]},"dependencies":{
            "@juicesharp/rpiv-todo":"2.15.0","@mozilla/readability":"0.6.0",
            "extra-skills":"1.0.0","missing-package":"1.0.0","pi-web-search":"1.7.0"}}),
    );
    write(
        "node_modules/@juicesharp/rpiv-todo",
        serde_json::json!({"pi":{"extensions":["./index.ts"]}}),
    );
    write(
        "node_modules/@mozilla/readability",
        serde_json::json!({"name":"@mozilla/readability"}),
    );
    write(
        "node_modules/extra-skills",
        serde_json::json!({"keywords":["pi-package"]}),
    );
    write(
        "node_modules/pi-web-search",
        serde_json::json!({"pi":{"extensions":["./src/index.ts"]}}),
    );
    d
}

fn pi_extension_args(args: &[String]) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == "--extension")
        .map(|w| w[1].clone())
        .collect()
}

#[test]
fn pi_openrouter_passes_provider_with_or_without_model_and_uses_own_key() {
    let cfg = AgentConfig {
        provider: "openrouter".into(),
        ..Default::default()
    };
    let packages = pi_extensions_fixture();
    for model in ["anthropic/claude-sonnet-4", ""] {
        let session = NativeSession {
            provider: String::new(), base_url: String::new(),
runtime: "pi".into(),
            native_id: "openrouter-session".into(),
            model: model.into(),
            turns: 0,
        };
        let args = agents::pi_arguments(
            &session,
            &cfg,
            Path::new("/tmp/pi-sessions"),
            packages.path(),
        );
        let index = args.iter().position(|arg| arg == "--provider").unwrap();
        assert_eq!(args[index + 1], "openrouter");
        // OpenRouter has no provider-native search: no web_search tool or extension.
        let tools = &args[args.iter().position(|a| a == "--tools").unwrap() + 1];
        assert!(tools.split(',').any(|t| t == "+web_fetch"));
        assert!(!tools.contains("web_search"));
        assert!(!tools.contains("codemode"));
        assert!(!args.iter().any(|a| a.contains("pi-web-search")));
        assert!(
            pi_extension_args(&args)
                .iter()
                .any(|a| a.ends_with("rpiv-todo"))
        );
        if !model.is_empty() {
            let index = args.iter().position(|arg| arg == "--model").unwrap();
            assert_eq!(args[index + 1], model);
        }
    }
    assert_eq!(
        crate::pi_providers::key_env("openrouter").unwrap(),
        "OPENROUTER_API_KEY"
    );
    assert_eq!(crate::pi_providers::key_env("openai").unwrap(), "OPENAI_API_KEY");
    assert_eq!(crate::pi_providers::key_env("anthropic").unwrap(), "ANTHROPIC_API_KEY");
}

#[test]
fn pi_resume_restores_write_shell_tools_and_keeps_session_model() {
    let cfg = AgentConfig {
        multi_provider: false,
        base_url: String::new(),
        api_key: "must-not-appear-in-argv".into(),
        provider: "anthropic".into(),
        effort: "high".into(),
        model: "changed-preference".into(),
        codemode: false,
    };
    let session = NativeSession {
        provider: String::new(), base_url: String::new(),
runtime: "pi".into(),
        native_id: "retained-session".into(),
        model: "opus".into(),
        turns: 12,
    };
    let packages = pi_extensions_fixture();
    let args = agents::pi_arguments(
        &session,
        &cfg,
        Path::new("/tmp/personal-sessions"),
        packages.path(),
    );
    let value = |flag: &str| args[args.iter().position(|a| a == flag).unwrap() + 1].as_str();
    assert_eq!(value("--session-id"), "retained-session");
    assert_eq!(value("--model"), "opus");
    assert_eq!(value("--provider"), "anthropic");
    assert_eq!(value("--thinking"), "high");
    // Only `+name` entries: the app's tools are forced on, user extension tools stay enabled.
    let tools: Vec<_> = value("--tools").split(',').collect();
    assert!(tools.iter().all(|t| t.starts_with('+')));
    for tool in "read,bash,edit,write,grep,find,ls,web_fetch,todo,web_search".split(',') {
        assert!(tools.contains(&format!("+{tool}").as_str()));
    }
    assert!(!tools.contains(&"+codemode"));
    let codemode = AgentConfig {
        codemode: true,
        ..cfg.clone()
    };
    let with_codemode = agents::pi_arguments(
        &session,
        &codemode,
        Path::new("/tmp/personal-sessions"),
        packages.path(),
    );
    let index = with_codemode.iter().position(|a| a == "--tools").unwrap();
    let codemode_tools: Vec<_> = with_codemode[index + 1].split(',').collect();
    assert!(codemode_tools.contains(&"+codemode"));
    // Package directories are loaded whole; plain libraries and missing packages are skipped.
    let root = packages.path().to_string_lossy().into_owned();
    assert_eq!(
        pi_extension_args(&args),
        [
            root.clone(),
            format!("{root}/node_modules/@juicesharp/rpiv-todo"),
            format!("{root}/node_modules/extra-skills"),
            format!("{root}/node_modules/pi-web-search"),
        ]
    );
    // Without a pi-extensions manifest Pi still starts, with its built-in tools only.
    let missing = tempfile::tempdir().unwrap();
    assert!(
        pi_extension_args(&agents::pi_arguments(
            &session,
            &cfg,
            Path::new("/tmp/personal-sessions"),
            missing.path()
        ))
        .is_empty()
    );
    assert!(
        agents::explain_agent_failure(
            "pi",
            "Tool \"web_search\" conflicts with /app/pi-extensions/x".into()
        )
        .contains("pi remove")
    );
    assert_eq!(
        agents::explain_agent_failure("codex", "conflicts with".into()),
        "conflicts with"
    );
    assert!(
        !args
            .iter()
            .any(|a| a.contains(&cfg.api_key) || a == "--no-session" || a == "--continue")
    );
}

#[test]
fn ci_fingerprint_covers_binary_files_and_modes_but_ignores_build_cache() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("target")).unwrap();
    let file = d.path().join("fixture.bin");
    std::fs::write(&file, [0, 255, 12]).unwrap();
    let original = agents::workspace_fingerprint(d.path()).unwrap();
    std::fs::write(d.path().join("target/cache"), "new cache").unwrap();
    assert_eq!(original, agents::workspace_fingerprint(d.path()).unwrap());
    std::fs::write(&file, [0, 255, 13]).unwrap();
    assert_ne!(original, agents::workspace_fingerprint(d.path()).unwrap());
    std::fs::write(&file, [0, 255, 12]).unwrap();
    assert_eq!(original, agents::workspace_fingerprint(d.path()).unwrap());
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_ne!(original, agents::workspace_fingerprint(d.path()).unwrap());
}

#[tokio::test]
async fn edits_after_real_ci_invalidate_the_gate_proof() {
    let d = tempfile::tempdir().unwrap();
    let a = app(d.path());
    let mut r = engine::create(
        &a,
        1,
        crate::presets::all().remove(0),
        "CI proof regression".into(),
        None,
    )
    .await
    .unwrap();
    let ci = r.workflow.node("tests").unwrap().clone();
    let output = crate::connectors::execute(&a, 1, &r, &ci).await.unwrap();
    assert!(output.summary.contains("test result: ok"));
    r.outputs.insert(ci.id.clone(), output);
    r.states.insert(ci.id.clone(), "completed".into());
    assert!(!engine::tests_current(&r).unwrap());
    r.context.insert(
        format!("test-proof:{}", ci.id),
        agents::workspace_fingerprint(Path::new(&r.workspace)).unwrap(),
    );
    assert!(engine::tests_current(&r).unwrap());
    let source = Path::new(&r.workspace).join("src/lib.rs");
    let mut content = std::fs::read_to_string(&source).unwrap();
    content.push_str("\n// A post-CI review edit\n");
    std::fs::write(source, content).unwrap();
    assert!(!engine::tests_current(&r).unwrap());
}

#[tokio::test]
async fn storage_examples_selection_creates_a_real_isolated_workspace() {
    let d = tempfile::tempdir().unwrap();
    let a = app(d.path());
    let mut w = crate::presets::all().remove(0);
    let source = w.nodes.iter_mut().find(|n| n.kind == "Source").unwrap();
    source.source_type = "examples".into();
    source.location = "helpdesk".into();
    let r = engine::create(&a, 1, w.clone(), "selected example".into(), None)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(Path::new(&r.workspace).join("src/lib.rs")).unwrap(),
        std::fs::read(a.assets.join("examples/helpdesk/src/lib.rs")).unwrap()
    );
    std::fs::write(Path::new(&r.workspace).join("src/lib.rs"), "run changes").unwrap();
    assert_ne!(
        std::fs::read(Path::new(&r.workspace).join("src/lib.rs")).unwrap(),
        std::fs::read(a.assets.join("examples/helpdesk/src/lib.rs")).unwrap()
    );
    assert!(crate::storage::source_folder(&a, "examples", "../src").is_err());
    assert!(crate::storage::source_folder(&a, "examples", "/etc").is_err());
    assert!(crate::storage::source_folder(&a, "unknown", "").is_err());
    assert!(crate::storage::source_folder(&a, "examples", "helpdesk/Cargo.toml").is_err());
}

#[test]
fn workflow_creation_time_is_server_owned_immutable_and_legacy_safe() {
    let dir = tempfile::tempdir().unwrap();
    let a = app(dir.path());
    let mut w = crate::presets::all().remove(0);
    w.created = 1;
    a.store.save_workflow(1, &w).unwrap();
    let created = a.store.workflow(1, &w.id).unwrap().created;
    assert!(created > 1);
    w.created = created + 999;
    w.name = "Edited".into();
    a.store.save_workflow(1, &w).unwrap();
    assert_eq!(a.store.workflow(1, &w.id).unwrap().created, created);
    let mut legacy = serde_json::to_value(&w).unwrap();
    legacy.as_object_mut().unwrap().remove("created");
    legacy["id"] = serde_json::json!("legacy");
    a.store
        .db
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO workflows VALUES(?,?,?)",
            rusqlite::params!["legacy", 1, legacy.to_string()],
        )
        .unwrap();
    let old = a.store.workflow(1, "legacy").unwrap();
    assert_eq!(old.created, 0);
    a.store.save_workflow(1, &old).unwrap();
    assert_eq!(a.store.workflow(1, "legacy").unwrap().created, 0);
    w.id = "new-workflow".into();
    a.store.save_workflow(1, &w).unwrap();
    assert_eq!(a.store.workflows(1).unwrap()[0].id, "new-workflow");
}

#[test]
fn workflow_rename_changes_only_owned_name_and_preserves_creation_and_graph() {
    let dir = tempfile::tempdir().unwrap();
    let a = app(dir.path());
    let w = crate::presets::all().remove(0);
    a.store.save_workflow(1, &w).unwrap();
    a.store.save_workflow(2, &w).unwrap();
    let before = a.store.workflow(1, &w.id).unwrap();
    let updated = a
        .store
        .rename_workflow(1, &w.id, "  New workflow name  ")
        .unwrap();
    assert_eq!(updated.name, "New workflow name");
    assert_eq!(updated.created, before.created);
    let mut expected = serde_json::to_value(&before).unwrap();
    expected["name"] = serde_json::json!("New workflow name");
    assert_eq!(serde_json::to_value(updated).unwrap(), expected);
    assert_eq!(a.store.workflow(2, &w.id).unwrap().name, w.name);
    for name in ["   ", "invalid\nname", &"a".repeat(161)] {
        assert!(a.store.rename_workflow(1, &w.id, name).is_err());
    }
    assert!(a.store.rename_workflow(3, &w.id, "No access").is_err());
    assert!(a.store.rename_workflow(1, "missing", "Unknown").is_err());
}

#[tokio::test]
async fn history_filters_workflow_and_owner_before_applying_the_run_limit() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let workflows = crate::presets::all();
    let original = engine::create(
        &app,
        1,
        workflows[0].clone(),
        "History fixture".into(),
        None,
    )
    .await
    .unwrap();
    for _ in 0..105 {
        let mut other = original.clone();
        other.id = id();
        other.workflow = workflows[1].clone();
        app.store.save_run(1, &other).unwrap();
    }
    let mut foreign = original.clone();
    foreign.id = id();
    app.store.save_run(2, &foreign).unwrap();
    assert!(
        app.store
            .runs(1)
            .unwrap()
            .iter()
            .all(|r| r.workflow.id != original.workflow.id)
    );
    let selected = app.store.workflow_runs(1, &original.workflow.id).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, original.id);
    assert_eq!(
        app.store.workflow_runs(1, &workflows[1].id).unwrap().len(),
        100
    );
    assert_eq!(
        app.store.workflow_runs(2, &original.workflow.id).unwrap()[0].id,
        foreign.id
    );
    assert!(
        app.store
            .workflow_runs(1, "unknown-workflow")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn source_copy_settings_default_persist_validate_and_isolate_users() {
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());
    let legacy: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(legacy.source_copy_limit_mib, 10);
    let mut settings = app.store.settings(1).unwrap();
    settings.source_copy_limit_mib = 3;
    app.store.save_settings(1, &settings).unwrap();
    assert_eq!(
        app.store
            .settings(1)
            .unwrap()
            .source_copy_limit_bytes()
            .unwrap(),
        3 * 1024 * 1024
    );
    assert_eq!(app.store.settings(2).unwrap().source_copy_limit_mib, 10);
    settings.source_copy_limit_mib = 0;
    assert!(settings.source_copy_limit_bytes().is_err());
    settings.source_copy_limit_mib = u64::MAX;
    assert!(settings.source_copy_limit_bytes().is_err());
}

#[tokio::test]
async fn configured_source_copy_limit_controls_engine_and_size_preview() {
    use axum::extract::{Query, State};
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());
    let source = app.sources.join("material");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::File::create(source.join("material.bin"))
        .unwrap()
        .set_len(2 * 1024 * 1024 + 1)
        .unwrap();
    let mut w = crate::presets::all().remove(0);
    let n = w.nodes.iter_mut().find(|n| n.kind == "Source").unwrap();
    n.source_type = "folder".into();
    n.source_mode = "copy".into();
    n.location = "material".into();
    let mut settings = app.store.settings(1).unwrap();
    settings.source_copy_limit_mib = 2;
    app.store.save_settings(1, &settings).unwrap();
    let error = engine::create(&app, 1, w.clone(), "test".into(), None)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("2 MiB") && error.contains("material.bin"));
    let preview = crate::storage::source_size(
        State(app.clone()),
        User {
            id: 1,
            email: "test@nodo.local".into(),
        },
        Query(serde_json::from_value(serde_json::json!({"path":"material"})).unwrap()),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(preview["threshold_bytes"], 2 * 1024 * 1024);
    assert!(preview["copy_error"].as_str().unwrap().contains("2 MiB"));
    settings.source_copy_limit_mib = 3;
    app.store.save_settings(1, &settings).unwrap();
    let r = engine::create(&app, 1, w.clone(), "test".into(), None)
        .await
        .unwrap();
    assert_eq!(
        std::fs::metadata(Path::new(&r.workspace).join("material.bin"))
            .unwrap()
            .len(),
        2 * 1024 * 1024 + 1
    );
    settings.source_copy_limit_mib = 1;
    app.store.save_settings(1, &settings).unwrap();
    w.nodes
        .iter_mut()
        .find(|n| n.kind == "Source")
        .unwrap()
        .source_mode = "direct".into();
    let direct = engine::create(&app, 1, w, "test".into(), None)
        .await
        .unwrap();
    assert_eq!(
        Path::new(&direct.workspace),
        std::fs::canonicalize(source).unwrap()
    );
}

#[tokio::test]
async fn comments_persist_without_edges_and_never_become_execution_steps() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let mut w = crate::presets::all().remove(0);
    for i in 0..2 {
        w.nodes.push(Node {
            id: format!("note-{i}"),
            kind: "Comment".into(),
            title: "Commento".into(),
            text: format!("# Nota {i}\n**Materiale** e spiegazione"),
            width: 420.0,
            height: 260.0,
            x: 50.0 + 450.0 * i as f64,
            y: 250.0,
            markdown: true,
            ..Default::default()
        });
    }
    engine::validate(&w).unwrap();
    app.store.save_workflow(1, &w).unwrap();
    let stored = app.store.workflow(1, &w.id).unwrap();
    let note = stored.node("note-1").unwrap();
    assert!(note.markdown);
    assert_eq!(
        (note.width, note.height, note.x, note.y),
        (420.0, 260.0, 500.0, 250.0)
    );
    assert!(note.text.contains("**Materiale**"));
    let r = engine::create(&app, 1, stored, "test".into(), None)
        .await
        .unwrap();
    assert!(!r.states.contains_key("note-0"));
    assert!(!r.states.contains_key("note-1"));
    assert_ne!(r.current, "note-0");
    w.entry = "note-0".into();
    assert!(engine::validate(&w).is_err());
    w.entry = r.workflow.entry.clone();
    w.edges.push(Edge {
        from: "note-0".into(),
        to: w.entry.clone(),
        kind: "control".into(),
        outcome: String::new(),
    });
    assert!(engine::validate(&w).is_err());
    w.edges.pop();
    w.nodes.iter_mut().find(|n| n.id == "note-0").unwrap().width = f64::NAN;
    assert!(engine::validate(&w).is_err());
}

#[test]
fn host_provider_environment_routes_credentials_and_preserves_personal_precedence() {
    use std::collections::BTreeMap;
    let host: BTreeMap<_, _> = [
        ("OPENAI_API_KEY", "host-openai"),
        ("CODEX_API_KEY", "host-codex"),
        ("ANTHROPIC_API_KEY", "host-anthropic"),
        ("OPENROUTER_API_KEY", "host-openrouter"),
        ("XAI_API_KEY", "host-xai"),
        ("DEEPSEEK_API_KEY", "host-deepseek"),
        ("UNRELATED_SECRET", "never-inherit"),
        ("HOME", "/host-home"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    for (runtime, provider, key_name, expected) in [
        ("codex", "anthropic", "CODEX_API_KEY", "host-codex"),
        ("claude", "anthropic", "ANTHROPIC_API_KEY", "host-anthropic"),
        ("pi", "anthropic", "ANTHROPIC_API_KEY", "host-anthropic"),
        ("pi", "openai", "OPENAI_API_KEY", "host-openai"),
        ("pi", "openrouter", "OPENROUTER_API_KEY", "host-openrouter"),
        ("pi", "xai", "XAI_API_KEY", "host-xai"),
        ("pi", "deepseek", "DEEPSEEK_API_KEY", "host-deepseek"),
    ] {
        let mut cfg = AgentConfig {
            provider: provider.into(),
            ..Default::default()
        };
        let resolve = |cfg: &AgentConfig| {
            agents::resolve_provider_environment(runtime, cfg, |key| host.get(key).cloned())
        };
        assert_eq!(
            resolve(&cfg),
            BTreeMap::from([(key_name.to_string(), expected.to_string())])
        );
        assert!(agents::has_provider_credentials(&resolve(&cfg)));
        cfg.api_key = "personal-key".into();
        assert_eq!(resolve(&cfg)[key_name], "personal-key");
    }
    let cfg = AgentConfig::default();
    let fallback = agents::resolve_provider_environment("codex", &cfg, |key| {
        if key == "CODEX_API_KEY" {
            Some(String::new())
        } else {
            host.get(key).cloned()
        }
    });
    assert_eq!(fallback["CODEX_API_KEY"], "host-openai");
    assert!(agents::resolve_provider_environment("codex", &cfg, |_| None).is_empty());
    for runtime in ["git", "cargo", "twg", "python3"] {
        assert!(
            agents::resolve_provider_environment(runtime, &cfg, |key| host.get(key).cloned())
                .is_empty()
        );
    }
}

#[test]
fn host_bedrock_environment_keeps_model_aliases_and_excludes_direct_api_keys() {
    use std::collections::BTreeMap;
    let mut host: BTreeMap<_, _> = [
        ("CLAUDE_CODE_USE_BEDROCK", "1"),
        ("AWS_REGION", "eu-central-1"),
        ("AWS_DEFAULT_REGION", "eu-central-1"),
        ("AWS_BEARER_TOKEN_BEDROCK", "bedrock-token"),
        (
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "eu.anthropic.claude-opus-5[1M]",
        ),
        (
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "eu.anthropic.claude-sonnet-5[1M]",
        ),
        (
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "eu.anthropic.claude-haiku-4-5-20251001-v1:0",
        ),
        (
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "Claude Opus 5 - Bedrock EU",
        ),
        (
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "Claude Sonnet 5 - Bedrock EU",
        ),
        (
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
            "Claude Haiku 4.5 - Bedrock EU",
        ),
        ("ANTHROPIC_API_KEY", "wrong-provider-key"),
        ("OPENAI_API_KEY", "unrelated-provider-key"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let cfg = AgentConfig {
        api_key: "personal-direct-key".into(),
        ..Default::default()
    };
    let resolve = |host: &BTreeMap<String, String>| {
        agents::resolve_provider_environment("claude", &cfg, |key| host.get(key).cloned())
    };
    let env = resolve(&host);
    assert_eq!(env.len(), 10);
    assert_eq!(
        env["ANTHROPIC_DEFAULT_OPUS_MODEL"],
        host["ANTHROPIC_DEFAULT_OPUS_MODEL"]
    );
    assert_eq!(
        env["ANTHROPIC_DEFAULT_OPUS_MODEL_NAME"],
        "Claude Opus 5 - Bedrock EU"
    );
    assert!(!env.contains_key("ANTHROPIC_API_KEY"));
    assert!(agents::has_provider_credentials(&env));
    host.insert("AWS_BEARER_TOKEN_BEDROCK".into(), String::new());
    assert!(!agents::has_provider_credentials(&resolve(&host)));
    host.insert("AWS_ACCESS_KEY_ID".into(), "access".into());
    host.insert("AWS_SECRET_ACCESS_KEY".into(), "secret".into());
    assert!(agents::has_provider_credentials(&resolve(&host)));
    host.insert("CLAUDE_CODE_USE_BEDROCK".into(), "0".into());
    let direct = resolve(&host);
    assert_eq!(direct["ANTHROPIC_API_KEY"], "personal-direct-key");
    assert!(!direct.keys().any(|key| key.starts_with("AWS_")));
    let pi = agents::resolve_provider_environment("pi", &AgentConfig::default(), |key| {
        host.get(key).cloned()
    });
    assert_eq!(pi.len(), 1);
    assert_eq!(pi["ANTHROPIC_API_KEY"], "wrong-provider-key");
}

#[test]
fn host_claude_oauth_is_explicit_and_never_mixed_with_api_keys_or_bedrock() {
    let cfg = AgentConfig {
        api_key: "personal-direct-key".into(),
        ..Default::default()
    };
    let lookup = |key: &str| match key {
        "CLAUDE_CODE_OAUTH_TOKEN" => Some("subscription-token".into()),
        "ANTHROPIC_API_KEY" => Some("host-direct-key".into()),
        _ => None,
    };
    let oauth = agents::resolve_provider_environment("claude", &cfg, lookup);
    assert_eq!(oauth.len(), 1);
    assert_eq!(oauth["CLAUDE_CODE_OAUTH_TOKEN"], "subscription-token");
    assert!(agents::has_provider_credentials(&oauth));
    let bedrock = agents::resolve_provider_environment("claude", &cfg, |key| match key {
        "CLAUDE_CODE_USE_BEDROCK" => Some("1".into()),
        "AWS_BEARER_TOKEN_BEDROCK" => Some("aws-token".into()),
        _ => lookup(key),
    });
    assert!(!bedrock.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
    assert!(!bedrock.contains_key("ANTHROPIC_API_KEY"));
    assert!(agents::has_provider_credentials(&bedrock));
    for rt in ["pi", "codex", "git"] {
        let env = agents::resolve_provider_environment(rt, &cfg, lookup);
        assert!(!env.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
    }
}

#[test]
fn host_gateway_bearer_preserves_endpoint_model_and_excludes_personal_and_oauth_keys() {
    let cfg = AgentConfig {
        api_key: "personal-key".into(),
        ..Default::default()
    };
    let host = |key: &str| match key {
        "ANTHROPIC_BASE_URL" => Some("https://gateway.example/anthropic".into()),
        "ANTHROPIC_AUTH_TOKEN" => Some("gateway-token".into()),
        "ANTHROPIC_MODEL" => Some("gateway-model".into()),
        "ANTHROPIC_DEFAULT_OPUS_MODEL" => Some("gateway-opus[1M]".into()),
        "CLAUDE_CODE_OAUTH_TOKEN" => Some("unrelated-oauth-token".into()),
        "ANTHROPIC_API_KEY" => Some("unrelated-direct-key".into()),
        _ => None,
    };
    let env = agents::resolve_provider_environment("claude", &cfg, host);
    assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "gateway-token");
    assert_eq!(
        env["ANTHROPIC_BASE_URL"],
        "https://gateway.example/anthropic"
    );
    assert_eq!(env["ANTHROPIC_MODEL"], "gateway-model");
    assert_eq!(env["ANTHROPIC_DEFAULT_OPUS_MODEL"], "gateway-opus[1M]");
    assert!(!env.contains_key("ANTHROPIC_API_KEY"));
    assert!(!env.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
    assert!(agents::has_provider_credentials(&env));
    let pi = agents::resolve_provider_environment("pi", &AgentConfig::default(), host);
    assert!(!pi.contains_key("ANTHROPIC_AUTH_TOKEN"));
    assert!(!pi.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
}

#[test]
fn host_cloud_provider_modes_route_aws_vertex_foundry_and_codex_access_tokens() {
    let cfg = AgentConfig {
        api_key: "personal-direct-key".into(),
        ..Default::default()
    };
    for (flag, credentials) in [
        (
            "CLAUDE_CODE_USE_BEDROCK",
            vec![
                ("AWS_PROFILE", "operator"),
                ("AWS_CONFIG_FILE", "/credentials/config"),
            ],
        ),
        (
            "CLAUDE_CODE_USE_VERTEX",
            vec![
                ("GOOGLE_APPLICATION_CREDENTIALS", "/credentials/google.json"),
                ("ANTHROPIC_VERTEX_PROJECT_ID", "test-project"),
                ("CLOUD_ML_REGION", "europe-west1"),
            ],
        ),
        (
            "CLAUDE_CODE_USE_FOUNDRY",
            vec![
                ("ANTHROPIC_FOUNDRY_AUTH_TOKEN", "foundry-token"),
                ("ANTHROPIC_FOUNDRY_RESOURCE", "test-resource"),
            ],
        ),
        (
            "CLAUDE_CODE_USE_ANTHROPIC_AWS",
            vec![
                ("ANTHROPIC_AWS_API_KEY", "aws-platform-key"),
                ("ANTHROPIC_AWS_WORKSPACE_ID", "test-workspace"),
            ],
        ),
    ] {
        let mut host = std::collections::BTreeMap::from([(flag, "true")]);
        host.extend(credentials);
        host.insert("CLAUDE_CODE_OAUTH_TOKEN", "oauth-must-not-leak");
        host.insert("ANTHROPIC_API_KEY", "direct-must-not-leak");
        let env = agents::resolve_provider_environment("claude", &cfg, |key| {
            host.get(key).map(|v| (*v).into())
        });
        assert!(agents::uses_cloud_provider(&env));
        assert!(agents::has_provider_credentials(&env));
        assert!(!env.contains_key("ANTHROPIC_API_KEY"));
        assert!(!env.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
    }
    let conflict = agents::resolve_provider_environment("claude", &cfg, |key| match key {
        "CLAUDE_CODE_USE_BEDROCK" | "CLAUDE_CODE_USE_VERTEX" => Some("1".into()),
        "AWS_BEARER_TOKEN_BEDROCK" | "GOOGLE_OAUTH_ACCESS_TOKEN" => Some("token".into()),
        _ => None,
    });
    assert!(!agents::has_provider_credentials(&conflict));
    let codex =
        agents::resolve_provider_environment("codex", &AgentConfig::default(), |key| match key {
            "CODEX_ACCESS_TOKEN" => Some("codex-access-token".into()),
            "OPENAI_BASE_URL" => Some("https://gateway.example/openai/v1".into()),
            _ => None,
        });
    assert_eq!(codex["CODEX_ACCESS_TOKEN"], "codex-access-token");
    assert!(agents::has_provider_credentials(&codex));
    assert!(!codex.contains_key("CODEX_API_KEY"));
}

#[test]
fn host_gateway_auth_headers_do_not_mix_with_personal_api_keys() {
    let cfg = AgentConfig {
        api_key: "personal-key".into(),
        ..Default::default()
    };
    let env = agents::resolve_provider_environment("claude", &cfg, |key| match key {
        "ANTHROPIC_BASE_URL" => Some("https://gateway.example".into()),
        "ANTHROPIC_CUSTOM_HEADERS" => {
            Some("X-Team: example\nAuthorization: Bearer gateway-token".into())
        }
        _ => None,
    });
    assert!(agents::has_provider_credentials(&env));
    assert!(!env.contains_key("ANTHROPIC_API_KEY"));
}

#[tokio::test]
async fn immediate_stop_drops_local_process_group_including_descendants() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let pid_file = dir.path().join("child.pid");
    {
        let work = agents::tool(
            &app,
            1,
            dir.path(),
            "bash",
            &["-c", "sleep 60 & echo $! > child.pid; wait"],
            90,
        );
        tokio::pin!(work);
        tokio::select! {
            result = &mut work => panic!("Process finished before cancellation: {result:?}"),
            _ = async {
                tokio::time::timeout(std::time::Duration::from_secs(3), async {
                    while !pid_file.exists() {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }).await.unwrap();
            } => {}
        }
        // Same drop path used by the worker's immediate-stop select.
    }
    let pid: i32 = std::fs::read_to_string(pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while unsafe { libc::kill(pid, 0) } == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("descendant survived cancellation");
}

#[test]
fn file_preview_settings_default_persist_validate_and_are_personal() {
    let dir=tempfile::tempdir().unwrap();let app=app(dir.path());
    let legacy:Settings=serde_json::from_str("{}").unwrap();assert_eq!(legacy.file_preview_limit_mb,5);assert_eq!(legacy.file_preview_limit_bytes().unwrap(),5_000_000);
    let mut settings=app.store.settings(1).unwrap();settings.file_preview_limit_mb=2;app.store.save_settings(1,&settings).unwrap();
    let reopened=Store::open(dir.path()).unwrap();assert_eq!(reopened.settings(1).unwrap().file_preview_limit_bytes().unwrap(),2_000_000);assert_eq!(reopened.settings(2).unwrap().file_preview_limit_mb,5);
    for invalid in [0,101,u64::MAX] {settings.file_preview_limit_mb=invalid;assert!(settings.file_preview_limit_bytes().is_err());}
}

#[tokio::test]
async fn storage_preview_respects_five_mb_boundary_and_personal_changes() {
    use axum::extract::{Query,State};
    let dir=tempfile::tempdir().unwrap();let app=app(dir.path());std::fs::create_dir_all(&app.sources).unwrap();
    let path=app.sources.join("large.txt");std::fs::write(&path,vec![b'x';5_000_000]).unwrap();
    async fn read(app:&App,uid:i64,path:&str)->serde_json::Value {
        let query=serde_json::from_value(serde_json::json!({"root":"sources","path":path})).unwrap();
        crate::storage::list(State(app.clone()),User{id:uid,email:String::new()},Query(query)).await.unwrap().0
    }
    let view=read(&app,1,"large.txt").await;assert_eq!(view["preview"].as_str().unwrap().len(),5_000_000);assert_eq!(view["preview_limit_mb"],5);
    let mut settings=app.store.settings(1).unwrap();settings.file_preview_limit_mb=1;app.store.save_settings(1,&settings).unwrap();
    let view=read(&app,1,"large.txt").await;assert!(view["preview"].is_null());assert_eq!(view["preview_reason"],"size");assert_eq!(view["preview_limit_mb"],1);
    assert!(read(&app,2,"large.txt").await["preview"].is_string());
    std::fs::write(&path,vec![b'x';5_000_001]).unwrap();assert_eq!(read(&app,2,"large.txt").await["preview_reason"],"size");
    settings.file_preview_limit_mb=6;app.store.save_settings(1,&settings).unwrap();assert!(read(&app,1,"large.txt").await["preview"].is_string());
    std::fs::write(app.sources.join("binary.dat"),b"\0\xffbinary").unwrap();assert_eq!(read(&app,1,"binary.dat").await["preview_reason"],"format");
}

#[test]
fn markdown_tables_preserve_content_without_executable_html() {
    let html=crate::api::render_markdown("| Progetto | Evidenza |\n|---|---|\n| CheckIbanDomiEstera | `File.java:41` |\n\n```html\n<table>code sample</table>\n```\n<script>alert(1)</script><img src=x onerror=alert(1)>");
    assert_eq!(html.matches("<table>").count(),1);
    assert!(html.contains("<table><thead>"));assert!(html.contains("CheckIbanDomiEstera"));assert!(html.contains("&lt;table&gt;code sample&lt;/table&gt;"));
    assert!(!html.contains("<script"));assert!(!html.contains("onerror"));
}

#[test]
fn pi_xai_and_deepseek_select_native_provider() {
    for (provider,key) in [("xai","XAI_API_KEY"),("deepseek","DEEPSEEK_API_KEY")] {
        let cfg = AgentConfig {provider:provider.into(),model:"selected-model".into(),..Default::default()};
        let session = NativeSession {provider: String::new(), base_url: String::new(),
runtime:"pi".into(),native_id:"provider-test".into(),model:cfg.model.clone(),turns:0};
        let args=agents::pi_arguments(&session,&cfg,std::path::Path::new("/tmp/session"),std::path::Path::new("/app/pi-extensions"));
        let index=args.iter().position(|arg|arg=="--provider").unwrap();assert_eq!(args[index+1],provider);
        assert_eq!(crate::pi_providers::key_env(provider).unwrap(),key);
    }
}

#[test]
fn manager_public_failures_never_expose_provider_payloads() {
    let cases = [
        (r#"{"type":"turn.failed","error":{"message":"unexpected status 401 Unauthorized: Incorrect API key provided: fake-partial****************, url: https://provider.invalid/responses, request id: secret-request"}}"#, "credenziali"),
        ("403 Forbidden", "permessi"),
        ("429 Too Many Requests", "limitando"),
        ("429 insufficient_quota", "quota"),
        ("404 model_not_found", "modello"),
        ("request timed out", "tempo"),
        ("fetch failed: connection refused", "raggiungere"),
        ("503 overloaded", "temporaneamente"),
        ("arbitrary failure fake-partial", "configurazione"),
    ];
    for (raw, expected) in cases {
        let message = agents::public_agent_failure(raw);
        assert!(message.contains(expected), "{message}");
        for sensitive in ["fake-partial", "secret-request", "https://", "turn.failed"] {
            assert!(!message.contains(sensitive));
        }
    }
}

#[test]
fn manager_write_permissions_default_on_persist_and_are_personal() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let legacy: Settings = serde_json::from_str("{}").unwrap();
    assert!(legacy.manager_write_enabled);
    let mut settings = app.store.settings(1).unwrap();
    assert!(settings.manager_write_enabled);
    let explicitly_disabled: Settings = serde_json::from_str(r#"{"manager_write_enabled":false}"#).unwrap();
    assert!(!explicitly_disabled.manager_write_enabled);
    settings.manager_write_enabled = true;
    app.store.save_settings(1, &settings).unwrap();
    let reopened = Store::open(dir.path()).unwrap();
    assert!(reopened.settings(1).unwrap().manager_write_enabled);
    assert!(reopened.settings(2).unwrap().manager_write_enabled);
    settings.manager_write_enabled = false;
    app.store.save_settings(1, &settings).unwrap();
    assert!(!app.store.settings(1).unwrap().manager_write_enabled);
}
