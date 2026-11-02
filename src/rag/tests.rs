use super::*;
#[tokio::test]
async fn swiftide_code_provenance_and_capabilities() {
    let cfg = Config::default();
    let text = "// à UTF-8\npub fn first() -> i32 { 1 }\n\npub fn second() -> i32 { first() }\n";
    let (chunks, _) = chunking::chunks("lib.rs", text, "hash", &cfg)
        .await
        .unwrap();
    assert!(!chunks.is_empty());
    for c in chunks {
        assert_eq!(c.text, text[c.byte_start..c.byte_end]);
        assert!(c.line_start <= c.line_end);
        assert!(c.metadata.get("outline").is_some());
    }
    assert_eq!(chunking::languages().len(), 14);
    for (path, text) in [
        ("a.go", "package main\nfunc main() {}"),
        ("a.c", "int main() { return 0; }"),
        ("a.html", "<html><body>Hello</body></html>"),
    ] {
        chunking::chunks(path, text, "h", &cfg).await.unwrap();
    }
}
#[tokio::test]
async fn lancedb_persistence_keyword_vectors_and_hybrid() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = chunking::provenance(
        "alpha.rs",
        "payment invoice processing",
        "a",
        0,
        26,
        "rust",
        json!({}),
    );
    a.vector = vec![1., 0., 0.];
    let mut b = chunking::provenance(
        "beta.rs",
        "customer profile settings",
        "b",
        0,
        25,
        "rust",
        json!({}),
    );
    b.vector = vec![0., 1., 0.];
    vector::write(dir.path(), &[a, b], true).await.unwrap();
    let all = vector::all(dir.path()).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(
        vector::chunk(dir.path(), &all[0].id).await.unwrap().text,
        all[0].text
    );
    assert!(vector::chunk(dir.path(), "invalid").await.is_err());
    for mode in ["keyword", "semantic", "hybrid"] {
        let q = Search {
            query: "invoice".into(),
            top_k: 2,
            ..Search::default()
        };
        let r = vector::search(dir.path(), &q, mode, Some(vec![1., 0., 0.]))
            .await
            .unwrap();
        assert_eq!(r[0]["path"], "alpha.rs");
    }
}
#[tokio::test]
async fn incremental_index_and_cancel_preserve_published_generation() {
    let dir = tempfile::tempdir().unwrap();
    let app = crate::tests::app(dir.path());
    let source = dir.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(
        source.join("a.md"),
        "# Hello\n\nAn invoice processing example.",
    )
    .unwrap();
    let mut i = Index {
        id: uuid::Uuid::new_v4().to_string(),
        root: "sources".into(),
        path: "test".into(),
        name: "test".into(),
        directory: true,
        config: Config {
            retrieval: "keyword".into(),
            ..Config::default()
        },
        created: now(),
        updated: now(),
        generation: None,
        generation_config: None,
        progress: Progress::default(),
        stats: Stats::default(),
        warnings: vec![],
    };
    let c = Arc::new(AtomicBool::new(false));
    let staging = base(&app, 1, &i.id)
        .unwrap()
        .join(uuid::Uuid::new_v4().to_string());
    run(&app, 1, &mut i, &source, &staging, false, &c)
        .await
        .unwrap();
    assert_eq!(i.stats.new_files, 1);
    let old = i.generation.clone();
    let mut pending = i.clone();
    c.store(true, Ordering::Relaxed);
    assert!(
        run(&app, 1, &mut pending, &source, &staging, false, &c)
            .await
            .is_err()
    );
    assert_eq!(get(&app, 1, &i.id).unwrap().generation, old);
    assert!(get(&app, 2, &i.id).is_err());
    c.store(false, Ordering::Relaxed);
    std::fs::write(
        source.join("a.md"),
        "# Updated\n\nInvoice reconciliation corrected.",
    )
    .unwrap();
    std::fs::write(source.join("b.txt"), "Unchanged customer information.").unwrap();
    let staging = base(&app, 1, &i.id)
        .unwrap()
        .join(uuid::Uuid::new_v4().to_string());
    run(&app, 1, &mut i, &source, &staging, false, &c)
        .await
        .unwrap();
    assert_eq!(i.stats.modified_files, 1);
    assert_eq!(i.stats.new_files, 1);
    std::fs::remove_file(source.join("a.md")).unwrap();
    let staging = base(&app, 1, &i.id)
        .unwrap()
        .join(uuid::Uuid::new_v4().to_string());
    run(&app, 1, &mut i, &source, &staging, false, &c)
        .await
        .unwrap();
    assert_eq!(i.stats.deleted_files, 1);
    assert_eq!(i.stats.reused_files, 1);
    assert_eq!(i.stats.reused_chunks, 1);
    assert_eq!(
        vector::all(&generation(&app, 1, &i).unwrap())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(i.progress.state, "completed");
}
#[test]
fn source_rejects_traversal_and_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let app = crate::tests::app(dir.path());
    assert!(crate::storage::rag_source(&app, 1, "examples", "../Cargo.toml").is_err());
    assert!(crate::storage::rag_source(&app, 1, "workspaces", "other-user").is_err());
    let source = dir.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", source.join("secrets.txt")).unwrap();
    let (m, _, skipped) = scan(&source, &Config::default(), &AtomicBool::new(false)).unwrap();
    assert!(m.files.is_empty());
    assert_eq!(skipped, 1);
    let file = dir.path().join("Dockerfile");
    std::fs::write(&file, "FROM rust:1.95\n").unwrap();
    let (m, _, _) = scan(&file, &Config::default(), &AtomicBool::new(false)).unwrap();
    assert_eq!(m.files.len(), 1);
}
#[tokio::test]
#[ignore = "Downloads real public ONNX model on first run"]
async fn real_fastembed_embeddings_and_token_splitting() {
    let cfg = Config {
        model: "multilingual-minilm".into(),
        encoder_tokens: 64,
        ..Config::default()
    };
    let engine = embedding::load(&cfg, std::path::Path::new("/tmp/nodo-rag-test-models")).unwrap();
    let text = "Invoice payments and accounting. àèìòù\n".repeat(100);
    let c = chunking::provenance("a.txt", &text, "h", 0, text.len(), "", json!({}));
    let (chunks, splits) = embedding::fit(&engine, vec![c], &cfg).unwrap();
    assert_eq!(splits, 1);
    assert_eq!(
        chunks.iter().map(|c| c.text.as_str()).collect::<String>(),
        text
    );
    let mut tokenizer = engine.tokenizer.clone();
    tokenizer.with_truncation(None).unwrap();
    for c in &chunks {
        assert!(tokenizer.encode(c.text.as_str(), true).unwrap().len() <= cfg.encoder_tokens);
    }
    let vectors = engine
        .embed(chunks.iter().map(|c| c.text.as_str()).collect(), Some(8))
        .unwrap();
    assert_eq!(vectors.len(), chunks.len());
    assert_eq!(vectors[0].len(), 384);
    assert!(vectors.iter().flatten().all(|f| f.is_finite()));
    drop(engine);
    for (id, dims) in [("bge-small-en", 384), ("jina-code", 768)] {
        let cfg = Config {
            model: id.into(),
            ..Config::default()
        };
        let engine =
            embedding::load(&cfg, std::path::Path::new("/tmp/nodo-rag-test-models")).unwrap();
        let v = embedding::query(
            &engine,
            "Invoice reconciliation in the payment service".into(),
            &cfg,
        )
        .unwrap();
        assert_eq!(v.len(), dims);
        assert!(v.iter().all(|f| f.is_finite()));
    }
}
#[tokio::test]
async fn queued_progress_cancel_and_duplicate_job() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = crate::tests::app(dir.path());
    app.assets = dir.path().join("assets");
    std::fs::create_dir_all(app.assets.join("examples/sample")).unwrap();
    std::fs::write(
        app.assets.join("examples/sample/a.md"),
        "# Example\n\nIndexed content.",
    )
    .unwrap();
    let serial = app.rag.serial.lock().await;
    let req = || Start {
        root: "examples".into(),
        path: "sample".into(),
        config: Config {
            retrieval: "keyword".into(),
            ..Config::default()
        },
        rebuild: false,
    };
    let user = || User {
        id: 1,
        email: "test@example.invalid".into(),
    };
    let Json(i) = start(State(app.clone()), user(), Json(req()))
        .await
        .unwrap();
    assert_eq!(i.progress.state, "queued");
    assert!(
        start(State(app.clone()), user(), Json(req()))
            .await
            .is_err()
    );
    assert!(get(&app, 2, &i.id).is_err());
    let _ = cancel(State(app.clone()), user(), Path(i.id.clone()))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if get(&app, 1, &i.id).unwrap().progress.state == "cancelled" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await
        }
    })
    .await
    .unwrap();
    assert!(get(&app, 1, &i.id).unwrap().generation.is_none());
    drop(serial);
}
#[tokio::test]
async fn rag_internal_tools_add_to_native_agent_inventory_with_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let app = crate::tests::app(dir.path());
    let source = dir.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("a.md"), "# Payments\n\nInvoice reconciliation.").unwrap();
    let mut i = Index {
        id: uuid::Uuid::new_v4().to_string(),
        root: "sources".into(),
        path: "source".into(),
        name: "Source".into(),
        directory: true,
        config: Config {
            retrieval: "keyword".into(),
            ..Config::default()
        },
        created: now(),
        updated: now(),
        generation: None,
        generation_config: None,
        progress: Progress::default(),
        stats: Stats::default(),
        warnings: vec![],
    };
    let generation = base(&app, 1, &i.id)
        .unwrap()
        .join(uuid::Uuid::new_v4().to_string());
    run(
        &app,
        1,
        &mut i,
        &source,
        &generation,
        false,
        &Arc::new(AtomicBool::new(false)),
    )
    .await
    .unwrap();
    let mut workflow = crate::presets::all().remove(0);
    let agent = workflow.node(&workflow.entry).unwrap().agent.clone();
    let node = crate::model::Node {
        id: "knowledge".into(),
        kind: "RAG".into(),
        title: "Knowledge".into(),
        rag_index: i.id.clone(),
        ..Default::default()
    };
    workflow.nodes.push(node.clone());
    workflow.edges.push(crate::model::Edge {
        from: node.id.clone(),
        to: agent,
        kind: "context".into(),
        outcome: String::new(),
    });
    let r = crate::engine::create(&app, 1, workflow, String::new(), None)
        .await
        .unwrap();
    let prepared = crate::mcp_client::prepare(&app, 1, &r)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prepared.tools.len(), 4);
    let search = prepared
        .tools
        .iter()
        .find(|t| t.name.ends_with("rag_search"))
        .unwrap();
    let result = call(&app, 1, &node, &search.name, json!({"query":"invoice"}))
        .await
        .unwrap()
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("a.md"));
    assert!(
        call(&app, 2, &node, &search.name, json!({"query":"invoice"}))
            .await
            .is_err()
    );
    let other = crate::model::Node {
        id: "different".into(),
        ..node.clone()
    };
    assert!(
        call(&app, 1, &other, &search.name, json!({"query":"invoice"}))
            .await
            .unwrap()
            .is_none()
    );
}
