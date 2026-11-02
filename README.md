# RightCommit

Self-hosted, Rust-built orchestration for Claude Code, Codex and Pi Agent. Parallel workflows, full control over every commit, and FinOps that shows exactly what each agent and model costs.

RightCommit helps you organize software work with AI agents (Claude Code, Codex and Pi Agent). It combines a visual canvas for designing workflows with a Rust workflow engine that runs agents and tools, persists execution state and enforces quality gates with bounded rework.

Website: https://rightcommit.com

## How work gets done

Nothing is committed until three checks have passed:

1. **Automated checks** run by the server.
2. **A second opinion** from another agent (which can be a different one).
3. **Your approval.**

Only then is a local Git commit created. The ready-made workflows end with a local commit: they never push, merge or deploy.

## Features

- Visual canvas for designing workflows
- Rust workflow engine with persisted execution state
- Project Manager assistant that drafts or refines workflows and proposes changes you can apply or undo (it never starts, approves or changes a run)
- Integrations: GitHub, Azure DevOps, ServiceNow, Jira, Confluence, Bitbucket, plus MCP and A2A. Every write to an external system waits for your confirmation
- FinOps views of cost per step, agent and model, with reported and estimated costs kept separate
- Secrets encrypted with AES-256-GCM and never sent back to the browser

## Quick start

Prerequisites: Docker, and a key or login for at least one supported agent.

```bash
docker compose up -d --build
```

Then open http://localhost:8080 and create the first account (the first user becomes admin, public sign-up closes afterwards). In settings add your agent key or login and choose a model. Press **+**, pick a ready-made workflow (or ask the Project Manager) and run it.

The server runs as a single container and stores data in SQLite.

## Contributing

Pull requests are welcome. Before your first pull request is merged you must sign the Contributor License Agreement (CLA). When you open a pull request, a bot comments with a link to the agreement; you sign by replying to that comment with the sentence it asks for, using your GitHub account. This is a one-time step. See [CONTRIBUTING.md](CONTRIBUTING.md) and [CLA.md](CLA.md).

## License

RightCommit is licensed under the [Functional Source License, Version 1.1, Apache 2.0 Future License (FSL-1.1-ALv2)](LICENSE). Each version converts to Apache 2.0 two years after its release. Self-hosting is free; you only pay for your own AI model usage.
