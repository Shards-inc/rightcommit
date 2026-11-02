use crate::model::*;
fn node(id: &str, kind: &str, title: &str, phase: &str, x: f64, y: f64) -> Node {
    Node {
        id: id.into(),
        kind: kind.into(),
        title: title.into(),
        phase: phase.into(),
        x,
        y,
        max_rework: 2,
        ..Default::default()
    }
}
fn edge(from: &str, to: &str, kind: &str, outcome: &str) -> Edge {
    Edge {
        from: from.into(),
        to: to.into(),
        kind: kind.into(),
        outcome: outcome.into(),
    }
}
fn task(id: &str, title: &str, command: &str, prompt: &str, y: f64) -> Node {
    let mut n = node(id, "Task", title, "Specification", 440., y);
    n.agent = "author".into();
    n.command = command.into();
    n.prompt = prompt.into();
    n
}
pub fn all() -> Vec<Workflow> {
    vec![preset(false), preset(true)]
}
fn preset(incident: bool) -> Workflow {
    let mut src = node(
        "source",
        "Source",
        "Northstar Helpdesk",
        "Context",
        80.,
        100.,
    );
    src.source_type = "example".into();
    if incident {
        src.location = "baseline".into();
    }
    let author = node("author", "Agent", "SDD author", "Context", 260., 100.);
    let mut nodes = vec![src, author];
    let mut edges = vec![edge("source", "author", "context", "")];
    let mut chain: Vec<String> = Vec::new();
    if incident {
        let mut triage = node("triage-agent", "Agent", "Triage", "Context", 260., 300.);
        triage.runtime = "codex".into();
        nodes.push(triage);
        edges.push(edge("source", "triage-agent", "context", ""));
        edges.push(edge("triage-agent", "assess", "context", ""));
        let mut ticket = node(
            "ticket",
            "Connection",
            "Read request",
            "Triage",
            440.,
            100.,
        );
        ticket.provider = "local".into();
        ticket.operation = "read_ticket".into();
        ticket.location = "incident.json".into();
        nodes.push(ticket);
        let mut assess = task(
            "assess",
            "Triage SDD",
            "analyze",
            "Use the skill to assess the repository and existing specification without changing code. Read incident.json. If no initial spec exists, document the V1 behavior described in README as the baseline before classifying. Distinguish implementation defects from specification changes. The preloaded case is a V2 business request eligible for approval, not a bug against V1. decision must be spec_change, code_defect or needs_info.",
            300.,
        );
        assess.agent = "triage-agent".into();
        assess.phase = "Triage".into();
        nodes.push(assess);
        let mut gate = node(
            "g-triage",
            "Gate",
            "Classify request",
            "Triage",
            620.,
            300.,
        );
        gate.agent = "triage-agent".into();
        gate.outcomes = vec![
            "spec_change".into(),
            "code_defect".into(),
            "needs_info".into(),
        ];
        gate.prompt="Examine the triage report and incident.json. Choose spec_change if a documented business rule changes; code_defect if code violates the current spec; needs_info if facts are missing. Do not infer human approval.".into();
        nodes.push(gate);
        let mut business = node(
            "business",
            "Review",
            "Approve business change",
            "Triage",
            800.,
            300.,
        );
        business.mode = "human".into();
        business.prompt="Approve V2: oldest ticket and summed occurrences, but lowest numeric priority. A requirement change requires explicit approval.".into();
        nodes.push(business);
        let mut mem = node(
            "handoff",
            "Memory",
            "Context for SDD",
            "Triage",
            980.,
            300.,
        );
        mem.agent = "assess".into();
        mem.target = "author".into();
        nodes.push(mem);
        let mut delta = task(
            "spec",
            "Update living spec",
            "clarify",
            "Resume the existing specification: apply the approved V2 business change in incident.json. Update documents only: FR and acceptance criteria for lowest priority, oldest ID, summed occurrences, P1-on-P3 cases and ties. Do not change code or tests in this node: Implement follows planning and review. If Source has no spec.md yet, create a V1 baseline spec.md in specs/001-ticket-dedup, then update that same file to V2, documenting the change. Run the clarify skill on this living spec. Do not use an invented amend command.",
            500.,
        );
        delta.phase = "Specification".into();
        nodes.push(delta);
        chain.extend(
            [
                "ticket", "assess", "g-triage", "business", "handoff", "spec",
            ]
            .map(str::to_owned),
        );
        edges.push(edge("g-triage", "business", "control", "spec_change"));
        edges.push(edge("g-triage", "handoff", "control", "code_defect"));
    } else {
        nodes.push(task("constitution","Constitution","constitution","Define concise principles: minimal Rust library, no network dependencies, deterministic tests, spec before code. Complete the constitution template and identify necessary questions.",100.));
        nodes.push(task("spec","Specify","specify","Specify the feature described in README.md: deterministic ticket deduplication. V1 preserves the oldest ticket priority, oldest ID and summed occurrences. Explicitly distinguish the V1 feature from the future V2 request in incident.json. A single feature specs/001-ticket-dedup is sufficient. Do not implement yet.",300.));
        nodes.push(task("clarify","Clarify","clarify","Clarify the V1 specification. Follow the rules for all decisions already documented in README. If an actual decision is missing, return needs_input with questions instead of assuming an answer.",500.));
        chain.extend(["constitution", "spec", "clarify"].map(str::to_owned));
    }
    for (id, title, cmd, prompt, y) in [
        (
            "plan",
            "Plan",
            "plan",
            "Plan the current feature in the existing Rust library. No external services or new dependencies. Use existing artifacts and keep the plan proportional to the repository.",
            700.,
        ),
        (
            "checklist",
            "Checklist",
            "checklist",
            "Check completeness and verifiability of current requirements: ID, fingerprint, priority, occurrences, error cases and tests.",
            900.,
        ),
        (
            "tasks",
            "Tasks",
            "tasks",
            "Generate small, verifiable tasks from the current plan. Include RED/GREEN tests and regressions. Do not rewrite the entire project.",
            1100.,
        ),
        (
            "analyze",
            "Analyze",
            "analyze",
            "Analyze consistency across spec, plan and tasks. Explicitly identify blocking gaps; do not claim tests were run at this stage.",
            1300.,
        ),
    ] {
        nodes.push(task(id, title, cmd, prompt, y));
        chain.push(id.into());
    }
    let mut review = node(
        "design-review",
        "Review",
        "Design review",
        "Specification",
        620.,
        1300.,
    );
    review.agent = "author".into();
    review.mode = "agent_and_human".into();
    review.prompt="Check spec, plan, tasks, checklist and the analyze report. Look for concrete inconsistencies. Ask only for missing decisions; use decision rework for actionable gaps, otherwise pass. Human approval is then required.".into();
    nodes.push(review);
    chain.push("design-review".into());
    let mut ready = node(
        "ready",
        "Gate",
        "Ready for code",
        "Delivery",
        800.,
        1300.,
    );
    ready.agent = "author".into();
    ready.outcomes = vec!["pass".into(), "rework".into()];
    ready.prompt="Confirm that the design review is approved and artifacts agree. Return pass only if no blocking gaps remain, otherwise rework.".into();
    nodes.push(ready);
    chain.push("ready".into());
    let mut implement = task(
        "implement",
        "Implement",
        "implement",
        "Execute all tasks in the current specification. Actually modify files and run cargo test. Keep the library small. Do not push or create PRs; if a test fails, fix it or declare blocked.",
        1500.,
    );
    implement.phase = "Delivery".into();
    nodes.push(implement);
    chain.push("implement".into());
    let mut converge = task(
        "converge",
        "Converge",
        "converge",
        "Verify actual convergence between implementation and the current spec. Run cargo test; compare acceptance criteria and tasks. Close only tasks actually completed. Return decision pass if convergent, rework if more work is needed.",
        1700.,
    );
    converge.phase = "Delivery".into();
    nodes.push(converge);
    chain.push("converge".into());
    let mut converged = node(
        "converged",
        "Gate",
        "Convergence verified",
        "Delivery",
        0.,
        0.,
    );
    converged.agent = "author".into();
    converged.require_tasks_closed = true;
    converged.outcomes = vec!["pass".into(), "rework".into()];
    converged.prompt="Check the Converge report and code against the specification. Return pass only if tasks are actually closed and no gaps remain; otherwise rework.".into();
    nodes.push(converged);
    chain.push("converged".into());
    let mut ci = node(
        "tests",
        "Connection",
        "Local CI · cargo test",
        "CI and PR",
        620.,
        1700.,
    );
    ci.provider = "local".into();
    ci.operation = "test".into();
    nodes.push(ci);
    chain.push("tests".into());
    let mut code = node(
        "code-review",
        "Review",
        "Code review",
        "CI and PR",
        800.,
        1700.,
    );
    code.agent = "author".into();
    code.mode = "agent_and_human".into();
    code.prompt="Examine code and actual test evidence. Look for regressions, edge cases and deviations from the current specification. Return decision pass or rework; human approval remains separate.".into();
    nodes.push(code);
    chain.push("code-review".into());
    let mut quality = node("quality", "Gate", "Quality gate", "CI and PR", 980., 1700.);
    quality.agent = "author".into();
    quality.require_tests = true;
    quality.outcomes = vec!["pass".into(), "rework".into()];
    quality.prompt="Return pass only with actual passing tests and no blocking gaps in the current review. Do not invent cloud CI or releases.".into();
    nodes.push(quality);
    chain.push("quality".into());
    let mut commit = node(
        "commit",
        "Connection",
        "Verified commit",
        "Delivery",
        1160.,
        1700.,
    );
    commit.provider = "local".into();
    commit.operation = "commit".into();
    nodes.push(commit);
    chain.push("commit".into());
    for pair in chain.windows(2) {
        if pair[0] == "g-triage" {
            continue;
        }
        edges.push(edge(
            &pair[0],
            &pair[1],
            "control",
            if ["ready", "converged", "quality"].contains(&pair[0].as_str()) {
                "pass"
            } else {
                ""
            },
        ));
    }
    edges.push(edge("ready", "plan", "rework", "rework"));
    edges.push(edge("quality", "implement", "rework", "rework"));
    edges.push(edge("converged", "implement", "rework", "rework"));
    edges.push(edge(
        "author",
        if incident { "spec" } else { "constitution" },
        "context",
        "",
    ));
    // Compact rows keep the graph readable; positions remain editable per workflow.
    let offset = if incident { 3 } else { 2 };
    if let Some(t) = nodes.iter_mut().find(|n| n.id == "triage-agent") {
        t.x = 440.;
        t.y = 100.;
    }
    for (i, id) in chain.iter().enumerate() {
        if let Some(n) = nodes.iter_mut().find(|n| &n.id == id) {
            let k = i + offset;
            n.x = 80. + (k % 6) as f64 * 180.;
            n.y = 100. + (k / 6) as f64 * 200.;
        }
    }
    Workflow{execution:Default::default(),created:0,id:if incident{"incident-sdd"}else{"feature-sdd"}.into(),name:if incident{"Incident → Living spec → Delivery"}else{"Feature → SDD → Delivery"}.into(),description:"Official Spec Kit, real agents, human reviews and local tests. The commit node does not push, merge or deploy. Configure Connection for remote systems.".into(),entry:chain[0].clone(),nodes,edges}
}

/// Templates are independent of the user's saved workflows and the two seeded demos.
#[cfg(test)]
#[derive(serde::Serialize)]
pub struct Template {
    pub id: String,
    pub section: String,
    pub services: Vec<String>,
    pub setup: String,
    pub workflow: Workflow,
}

#[cfg(test)]
fn adversarial_baseline() -> Workflow {
    crate::templates::seed()
        .templates
        .into_iter()
        .find(|t| t.content.id == "as-is-sdd-adversarial")
        .unwrap()
        .content
        .workflow
}
#[cfg(test)]
pub fn catalog() -> Vec<Template> {
    crate::templates::seed()
        .templates
        .into_iter()
        .map(|t| Template {
            id: t.content.id,
            section: t.content.section,
            services: t.content.services,
            setup: t.content.locales["en"].setup.clone(),
            workflow: t.content.workflow,
        })
        .collect()
}

#[cfg(test)]
mod catalog_tests {
    #[test]
    fn templates_are_valid_and_independent_from_seeded_workflows() {
        let templates = super::catalog();
        assert_eq!(templates.len(), 11);
        let mut ids = std::collections::HashSet::new();
        for t in templates {
            assert!(ids.insert(t.id.clone()));
            crate::engine::validate(&t.workflow).unwrap_or_else(|e| panic!("{}: {e}", t.id));
            assert!(!t.setup.is_empty());
            for n in &t.workflow.nodes {
                if n.provider == "atlassian" {
                    crate::twg::arguments(&n.command).unwrap();
                }
            }
        }
        assert_eq!(super::all().len(), 2);
    }
    #[test]
    fn adversarial_baseline_preserves_the_customer_analysis_flow() {
        let w = super::adversarial_baseline();
        crate::engine::validate(&w).unwrap();
        assert_eq!(w.nodes.len(), 14);
        assert_eq!(w.edges.len(), 15);
        assert_eq!(w.entry, "task-55c0b357");
        assert_eq!(w.node("agent-aa47bf35").unwrap().runtime, "claude");
        assert_eq!(w.node("agent-codex").unwrap().runtime, "codex");
        assert_eq!(w.nodes.iter().filter(|n| n.kind == "Task").count(), 5);
        assert_eq!(w.node("gate-catalogo").unwrap().max_rework, 3);
        assert_eq!(w.next("gate-catalogo", "rework").unwrap().to, "task-verifica");
        assert_eq!(w.next("gate-catalogo", "pass").unwrap().to, "review-approvazione");
        assert_eq!(w.next("review-approvazione", "rework").unwrap().to, "task-perfeziona");
        assert_eq!(w.node("review-approvazione").unwrap().mode, "human");
        assert_eq!(w.node("mem-catalogo").unwrap().target, "agent-codex");
        assert_eq!(w.node("mem-verifica").unwrap().target, "agent-aa47bf35");
        assert_eq!(w.node("mem-perfeziona").unwrap().target, "agent-codex");
        let json = serde_json::to_string(&w).unwrap();
        for project_specific in ["EVIDENZE.md", "INBREVE.md", "CATALOGO_FUNZIONALE.md", "VERIFICA_CATALOGO.md", "claude-opus-5-5"] {
            assert!(!json.contains(project_specific));
        }
    }
}
