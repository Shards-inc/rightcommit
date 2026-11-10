<p align="center">
  <a href="https://rightcommit.com">
    <img src=".github/assets/rightcommit-icon.png" alt="rightcommit icon" width="128" height="128">
  </a>
</p>

<h1 align="center">
  <a href="https://rightcommit.com">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://github.com/rightcommit/rightcommit/raw/HEAD/.github/assets/rightcommit-wordmark-dark.svg?sanitize=true">
      <img src=".github/assets/rightcommit-wordmark-light.svg" alt="rightcommit" width="360" height="72">
    </picture>
  </a>
</h1>

English | [简体中文](README_zh-CN.md)

<!-- Keep README_zh-CN.md in sync when updating this README. -->

<p align="center">
  <strong>AI agents say “done”. rightcommit proves it.</strong>
</p>

<p align="center">
  Self-hosted orchestration for Claude Code, Codex, and Pi.<br>
  Describe what you want to automate and the <strong>Project Manager</strong> drafts
  an editable workflow on the visual canvas.
</p>

<p align="center">
  <a href="https://rightcommit.com"><strong>Website</strong></a> ·
  <a href="#quickstart"><strong>Quickstart</strong></a> ·
  <a href="#documentation"><strong>Docs</strong></a> ·
  <a href="#contributing"><strong>Contribute</strong></a> ·
  <a href="#license"><strong>License</strong></a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-6254e8" alt="License: Apache 2.0"></a>
  <img src="https://img.shields.io/badge/built%20with-Rust-b7410e" alt="Built with Rust">
  <a href="https://github.com/rightcommit/rightcommit/actions/workflows/docker-check.yml"><img src="https://github.com/rightcommit/rightcommit/actions/workflows/docker-check.yml/badge.svg?branch=main" alt="Docker image checks"></a>
  <a href="https://github.com/rightcommit/rightcommit/actions/workflows/package-check.yml"><img src="https://github.com/rightcommit/rightcommit/actions/workflows/package-check.yml/badge.svg?branch=main" alt="Installable package checks"></a>
  <a href="https://github.com/rightcommit/rightcommit/stargazers"><img src="https://img.shields.io/github/stars/rightcommit/rightcommit?style=social" alt="GitHub stars"></a>
</p>

---

![rightcommit demo showing the Project Manager drafting a workflow, then execution, tests, review, a local commit, and FinOps](.github/assets/rightcommit-walkthrough.gif)

## Quickstart

### Docker

Start Docker, then run the command below to clone and start rightcommit.

```sh
git clone https://github.com/rightcommit/rightcommit.git && cd rightcommit && docker compose up -d --build
```

The first build can take several minutes. For a ZIP download or step-by-step instructions, see the
[Docker install guide](INSTALL.md).

### Homebrew on macOS

```sh
brew install rightcommit/tap/rightcommit
mkdir -p ~/rightcommit && cd ~/rightcommit
rightcommit setup
rightcommit
```

Run `rightcommit setup` once to install the agents. Launch `rightcommit` from the same folder
on future starts to reuse your data. See the [macOS install guide](INSTALL_MAC.md).

### Run your first workflow

1. Open <http://localhost:8080>. The first account becomes the administrator, and public sign-up
   closes by default.
2. In **Personal settings → Agents**, configure an API key or authorized provider for at least
   one agent and pick a model. See [Configuring the agents](#configuring-the-agents).
3. Press **+** to pick a ready-made workflow, or describe one to the **Project Manager**. Check
   the draft on the canvas and run it.

**[Star rightcommit](https://github.com/rightcommit/rightcommit)** if you find it useful.
[Report a bug or suggest an idea](https://github.com/rightcommit/rightcommit/issues),
or see [how to contribute](CONTRIBUTING.md).

## Why rightcommit

- **Verified commits.** Build workflows that require passing tests, agent review, and human
  approval before a commit.
- **Coordinated agents.** Assign Claude Code, Codex, or Pi to different steps in the same workflow.
- **Transparent costs.** See costs and tokens by agent, model, and run in FinOps.

## What you get

- **Agentic workflow orchestration.** Design and run workflows that combine AI agents
  and tools. Workflows can be exposed as A2A-compatible agents.
- **Quality gates.** Check that tasks are closed, run tests on the server, and recheck
  results when files change after testing.
- **Traceable work.** Every step saves its files as versions with a SHA-256 fingerprint and
  a line-by-line diff. Failed checks send the work back for correction, within a set number
  of attempts.
- **A workflow that fits the job.** Three steps or a full delivery pipeline. Start from one
  of the ready-made workflows, such as Spec Kit specification → plan → tasks → implementation,
  or build your own.
- **You stay in control.** Run interactively, assisted or unattended. Pause for human
  decisions and resume where you left off.
- **Not only code.** The same blocks can run research, document analysis or reporting workflows.

## Connections

- **Agents:** Claude Code, Codex and Pi, with an API key or an authorized gateway (Anthropic,
  OpenAI, OpenRouter, AWS Bedrock, Google Vertex, Microsoft Foundry and more).
- **Services:** GitHub, Azure DevOps, ServiceNow, and Jira, Confluence and Bitbucket through
  the Atlassian CLI. Every write to an outside system waits for your confirmation of the
  exact content.
- **Open standards:** MCP to give agents tools from your MCP servers, and A2A to publish a
  workflow that other agents can call.

## Who it's for

rightcommit supports individual use and internal organizational deployments with separate
user accounts. Read [Provider terms and permitted use](PROVIDER_TERMS.md) and
[Intended use and third-party provider obligations](INTENDED_USE.md). Provider names identify integrations
and do not imply affiliation or endorsement.

- **Developers** running agents on their own laptop, with their own keys.
- **Small teams** who prefer to self-host, with separate users, keys and workspaces.

## Other ways to install

| Guide | For |
|---|---|
| [INSTALL.md](INSTALL.md) | Docker on macOS, Windows and Linux, step by step |
| [INSTALL_MAC.md](INSTALL_MAC.md) | macOS without containers: Homebrew, crates.io or source |
| [INSTALL_LINUX.md](INSTALL_LINUX.md) | Linux and WSL2 without containers: crates.io or source |
| [INSTALL_PODMAN.md](INSTALL_PODMAN.md) | Podman instead of Docker |

### Managing the Docker install

| What | Command |
|---|---|
| Stop the app | `docker compose down` |
| Update after getting a new version | `docker compose up -d --build` |
| Use another port | set `RIGHTCOMMIT_PORT=9090` in a `.env` file, then `docker compose up -d` |

> [!WARNING]
> Never run `docker compose down -v`: it deletes the `rightcommit-data` volume, with all
> accounts, workflows and keys.

By default, Docker Compose binds the app to localhost.

## Configuring the agents

Start with [Provider terms and permitted use](PROVIDER_TERMS.md). All three agents support multiple users;
provider authorization and credential scope still apply.

| Guide | Covers |
|---|---|
| [HOW_TO_CONFIGURE_CLAUDECODE.md](HOW_TO_CONFIGURE_CLAUDECODE.md) | Claude Code: API keys, gateways, Bedrock, Vertex, Foundry |
| [HOW_TO_CONFIGURE_CODEX.md](HOW_TO_CONFIGURE_CODEX.md) | Codex: API keys and compatible provider configuration |
| [HOW_TO_CONFIGURE_PI.md](HOW_TO_CONFIGURE_PI.md) | Pi with Anthropic or OpenAI |
| [HOW_TO_CONFIGURE_PI_OPENROUTER.md](HOW_TO_CONFIGURE_PI_OPENROUTER.md) | Pi with OpenRouter |
| [HOW_TO_CONFIGURE_PI_LOCAL.md](HOW_TO_CONFIGURE_PI_LOCAL.md) | Pi with a local model (Ollama, LM Studio, vLLM) |

## Documentation

| Document | What it covers |
|---|---|
| [CHANGELOG.md](CHANGELOG.md) | What changed in each version |
| [ADD_A_SKILL.md](ADD_A_SKILL.md) | Give a skill to Claude Code, Codex and Pi in one step |
| [ADD-PI-PACKAGES.md](ADD-PI-PACKAGES.md) | Add packages from pi.dev to the Pi agent |
| [ADD-OPENSPEC.md](ADD-OPENSPEC.md) | Use OpenSpec in your workflows |
| [UPDATE-PI-DEFAULT-PACKAGES.md](UPDATE-PI-DEFAULT-PACKAGES.md) | Update the Pi tools rightcommit ships (maintainers) |
| [CONFIGURATION GUIDES](guides/README.md) | Configure access and run workflow examples for ServiceNow (ITSM, catalog Requests and Spec Kit), Atlassian (Jira, Bitbucket and Confluence), and GitHub (issues and CI) |

## License

Copyright (c) 2026 Lorenzo Toscano

rightcommit is open source, released under the **[Apache License, Version 2.0](LICENSE)**.

You can use, modify, self-host and distribute the code, including for commercial purposes,
under the terms of the license. See [LICENSE](LICENSE) for the full terms.

The permissions above concern rightcommit itself. Any third-party product or service used
with it remains subject to its own terms and the [supported deployment scope](INTENDED_USE.md).

Third-party notices for bundled icons and Spec Kit files are in [`licenses/`](licenses).

## Contributing

Pull requests are welcome. Before your first one is merged you sign the
[Contributor License Agreement](CLA.md): a bot comments on the pull request, and you reply with
the sentence it asks for. You only need to do this once. See [CONTRIBUTING.md](CONTRIBUTING.md).

Found a bug or have an idea? [Open an issue](https://github.com/rightcommit/rightcommit/issues).

Questions and feedback: [hello@rightcommit.com](mailto:hello@rightcommit.com).

---

rightcommit is an independent side project created and maintained by Lorenzo Toscano in his personal capacity. It is developed in his own time, with his own resources, and is not affiliated with, endorsed by, or sponsored by his employer, any of its clients, or any other organization. Views and choices expressed in this project are his own.

<sub>Claude Code, Codex, Pi, GitHub, Azure DevOps, ServiceNow, Jira, Confluence, Bitbucket,
Spec Kit, Rust, Docker and other names and marks belong to their owners. rightcommit is not
affiliated with them.</sub>
