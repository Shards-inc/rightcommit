# Changelog

What changed in each version of rightcommit. Releases are also listed on
[GitHub](https://github.com/rightcommit/rightcommit/releases).

## [1.2.1] - 2026-11-05

### Added

- A **Filter** for the run's artifacts: choose which folders to show, in a tree you can
  expand and collapse. Hiding a folder also hides its subfolders, and the files are kept.
  `target/debug` folders are hidden by default.

### Changed

- New runs keep their internal files (reports and copied sources) in `.rightcommit` instead
  of `.nodo`. Runs saved with `.nodo` can still be opened, and a revision copies their
  sources into `.rightcommit`.
- The login form works with password managers like any other login, without a separate
  switch. The link under the registration form now reads **Back to login**.
- With Docker or Podman, `NODO_ALLOW_REGISTRATION`, `NODO_SECURE_COOKIE`,
  `NODO_CONNECTOR_HOSTS` and `RUST_LOG` can be set in `.env`. The defaults are unchanged:
  registration closed and cookies for local HTTP.
- `.env.example` groups the settings by purpose and documents the options for native
  installs. [INSTALL_MAC.md](INSTALL_MAC.md) explains where to put `.env` with Homebrew.
- The README is shorter and gives an overview of how to use rightcommit.

### Fixed

- A revision of a run is no longer refused when the Manager reorders the workflow or changes
  fields that do not affect its Sources.
- Error messages appear above open dialogs instead of behind them.

## [1.2.0] - 2026-11-04

### Added

- Users other than the administrator can use the administrator's saved API key. When a user
  has no personal key, Claude Code and Codex use the administrator key for the same agent,
  and Pi uses it when the provider and endpoint match. The key is never shown to the user,
  and Settings reports **Administrator API key** as the source. A personal key always takes
  precedence.
- The Manager has two permanent folders for each account, `manager/mcp-servers` and
  `manager/projects`. When writing is enabled in **Personal settings → Manager**, it builds
  requested MCP servers and projects there. They are kept when the workflow is deleted.
- Pi Agent nodes show the provider they use in the graph.
- [Provider terms and permitted use](PROVIDER_TERMS.md) and
  [Intended use](INTENDED_USE.md) explain the supported deployment scope and the provider
  terms that apply to each agent.

### Changed

- Provider configuration from the environment for Claude Code and Codex (gateways, cloud
  credentials and tokens in `.env`) applies only to the administrator. Other users need a
  personal API key or the administrator's saved key. Pi keeps reading provider variables
  from `.env` for every user.
- A saved API key for Claude Code or Codex always means direct API billing: gateways, cloud
  credentials and tokens in the environment no longer override it.
- API key fields for Claude Code, and for Pi with Anthropic, accept only Anthropic API keys,
  not Claude subscription tokens. Pi no longer uses `ANTHROPIC_AUTH_TOKEN`,
  `ANTHROPIC_OAUTH_TOKEN` or a Claude subscription login. If one is found, Pi stops with an
  error and leaves the file untouched.
- Removed the option to import the server's local Claude Code or Codex login into an account
  (`NODO_ALLOW_HOST_AUTH_IMPORT`). `.env.example` no longer lists `CODEX_ACCESS_TOKEN`,
  `ANTHROPIC_OAUTH_TOKEN` and `CLAUDE_CODE_OAUTH_TOKEN`.
- Agents and connections use labeled icons instead of provider logos. The footer notes that
  trademarks belong to their owners.
- The environment check shows native logins and login files as **External configuration**.
- The configuration guides for Claude Code, Codex and Pi list every supported Pi provider
  variable and explain credentials for multiple users.

### Fixed

- Error messages and compaction events shown to other users hide the administrator's key.
- Changing an Agent node's runtime or Pi provider updates the graph right away.
- The switch that hides the agent setup notice is centered with its label.

## [1.1.0] - 2026-11-04

### Added

- Scheduled runs. The **Run execution** menu offers **Run now** and **Scheduled**: a single
  start, or one every N minutes or hours, daily or weekly, with a time zone and an optional
  end date. Each schedule groups its runs. Pausing or deleting a schedule stops only future
  starts, and a start is skipped while the previous run is still open.
- A native ServiceNow connector. It reads incidents and catalog Requests, creates or reuses
  incidents, and updates and resolves tickets. It signs in with Basic, Bearer or OAuth Client
  Credentials, and the OAuth token is fetched automatically and reused until it expires.
  Writes still wait for approval.
- A **ServiceNow and ITSM** section in the template catalog:
  - ITSM ticket classification and resolution with Pi and an internal RAG index, in a generic
    version and a Northstar knowledge base demo. Ticket batches are deduplicated across runs.
  - Catalog Requests from configuration files.
  - A ServiceNow software requirement turned into a Spec Kit specification.
  - An OAuth connection test.
- GitHub API requests from a Connection node, with a searchable catalog of all 1,254 REST
  operations, plus GraphQL. POST, PUT, PATCH and DELETE requests wait for approval.
- New templates under **SDLC with Connections**:
  - GitHub: connection test, issue to requirements and tests, commit CI readiness, and demo
    CI setup.
  - Atlassian: Jira issue to requirements and tests, Jira + Bitbucket PR review dossier, and
    Confluence page to SDLC briefing.
- Connection nodes can set their own organization, project and repository instead of the
  personal defaults, and show the provider's icon.
- Atlassian sign-in from Personal settings with browser OAuth, scoped Bitbucket tokens and a
  **Test connection** button.
- A **Multi-provider** mode for Pi. Each Pi Agent node picks a provider from Pi's catalog
  (42 providers in Pi 1.4.0) and a model. Credentials come from `.env` or native Pi logins.
  Local models work through `PI_LOCAL_BASE_URL`.
- Outside Docker, rightcommit loads `.env` from the current folder at startup. Variables that
  are already set take precedence.
- How-to guides for ServiceNow, GitHub and Atlassian in [guides/](guides/README.md).

### Changed

- The interface starts in English on first launch. An Italian choice you saved is kept.
- The login card has a cleaner wordmark.
- The notice about agents that are not configured can be turned off, separately in each
  browser.
- The README has a demo image and describes orchestration, scheduled runs and Pi with
  multiple providers.
- The package check on GitHub skips changes that only touch documentation and caches Rust
  builds.

### Fixed

- Settings are protected from browser and password-manager autofill. Autofill that nobody
  asked for is ignored, and saving is blocked if a ServiceNow credential lands in an agent
  field.

## [1.0.2] - 2026-11-02

### Added

- Homebrew bottles for Apple Silicon Macs with macOS 15 or later: `brew install` and
  `brew upgrade` download rightcommit ready to use instead of compiling it.
- A new user starts on an empty workflow instead of the first example. The examples are still
  one click away under **+**.

### Changed

- The login card has a centered header, and the rightcommit wordmark no longer leaves a gap
  before the logo.
- The Docker image compiles its dependencies in a separate layer, so rebuilding after a code
  change takes about a minute.

## [1.0.1] - 2026-11-02

### Added

- Outside Docker, rightcommit creates and uses `./sources` next to `./data`, so Homebrew,
  crates.io and source installations no longer need `NODO_SOURCES` at every start.
- The agent versions live in one file, `distribution/agents.env`, used by both the Docker
  image and `rightcommit setup`.
- The automatic check on GitHub also builds and starts the Docker image.

### Changed

- Node.js 24 for the Docker image, the Homebrew formula and the guides (22.19 or newer still
  works).
- Codex 0.166.0 and Claude Code 2.1.316. Pi stays at 1.4.0.
- Homebrew needs Rust only to compile rightcommit, so it no longer takes the place of rustup.
- The macOS and Linux guides install with Homebrew and crates.io.

### Fixed

- pulldown-cmark 0.13.6, for [RUSTSEC-2026-0433](https://rustsec.org/advisories/RUSTSEC-2026-0433).

## [1.0.0] - 2026-11-02

First public release.

- Visual workflow canvas, with a Project Manager that drafts workflows from a description.
- Claude Code, Codex and Pi in one workspace, each step with its own agent and model.
- Verified outcomes: task lists, tests run on the server, review by a second agent and human
  approval before a commit.
- Versioned artifacts with SHA-256 fingerprints, line-by-line diffs and bounded rework.
- FinOps panel with costs and tokens per step, run, workflow, agent and model.
- Connections to GitHub, Azure DevOps, ServiceNow and Atlassian, plus MCP and A2A.
- Runs with Docker or Podman, or natively on macOS and Linux.
