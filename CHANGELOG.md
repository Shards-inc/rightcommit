# Changelog

What changed in each version of rightcommit. Releases are also listed on
[GitHub](https://github.com/rightcommit/rightcommit/releases).

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
