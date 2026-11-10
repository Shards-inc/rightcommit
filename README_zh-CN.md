<p align="center">
  <a href="https://rightcommit.com">
    <img src=".github/assets/rightcommit-icon.png" alt="rightcommit 图标" width="128" height="128">
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

[English](README.md) | 简体中文

<!-- 更新 README.md 时，请同步更新本文件。 -->

<p align="center">
  <strong>AI agents say “done”. rightcommit proves it.</strong><br>
  <strong>AI 智能体说“完成了”，rightcommit 来证明。</strong>
</p>

<p align="center">
  面向 Claude Code、Codex 和 Pi 的自托管编排工具。<br>
  描述要自动化的任务，<strong>Project Manager</strong>（项目经理）会在可视化画布上生成可编辑的工作流草稿。
</p>

<p align="center">
  <a href="https://rightcommit.com"><strong>官网</strong></a> ·
  <a href="#quickstart"><strong>快速开始</strong></a> ·
  <a href="#documentation"><strong>文档</strong></a> ·
  <a href="#contributing"><strong>参与贡献</strong></a> ·
  <a href="#license"><strong>许可证</strong></a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-6254e8" alt="许可证：Apache-2.0"></a>
  <img src="https://img.shields.io/badge/built%20with-Rust-b7410e" alt="使用 Rust 构建">
  <a href="https://github.com/rightcommit/rightcommit/actions/workflows/docker-check.yml"><img src="https://github.com/rightcommit/rightcommit/actions/workflows/docker-check.yml/badge.svg?branch=main" alt="Docker 镜像检查"></a>
  <a href="https://github.com/rightcommit/rightcommit/actions/workflows/package-check.yml"><img src="https://github.com/rightcommit/rightcommit/actions/workflows/package-check.yml/badge.svg?branch=main" alt="安装包检查"></a>
  <a href="https://github.com/rightcommit/rightcommit/stargazers"><img src="https://img.shields.io/github/stars/rightcommit/rightcommit?style=social" alt="GitHub Star 数"></a>
</p>

---

![rightcommit 演示：Project Manager 生成工作流草稿，随后执行、测试、审查、本地提交并查看 FinOps](.github/assets/rightcommit-walkthrough.gif)

<a name="quickstart"></a>

## 快速开始

### Docker

启动 Docker，然后运行以下命令克隆并启动 rightcommit。

```sh
git clone https://github.com/rightcommit/rightcommit.git && cd rightcommit && docker compose up -d --build
```

首次构建可能需要几分钟。下载 ZIP 或查看分步说明，请参阅 [Docker 安装指南](INSTALL.md)。

<!-- TODO: link Gitee
如果在中国大陆访问 GitHub 较慢，可在 Gitee 镜像上线后使用该镜像。
-->

### 在 macOS 上使用 Homebrew

```sh
brew install rightcommit/tap/rightcommit
mkdir -p ~/rightcommit && cd ~/rightcommit
rightcommit setup
rightcommit
```

首次运行 `rightcommit setup` 以安装智能体。以后从同一目录启动 `rightcommit`，即可继续使用已有数据。参见 [macOS 安装指南](INSTALL_MAC.md)。

### 运行第一个工作流

1. 打开 <http://localhost:8080>。首个账户将成为管理员，此后默认关闭公开注册。
2. 在 **Personal settings → Agents** 中，为至少一个智能体配置 API 密钥或已获授权的服务提供方，并选择模型。参见[配置智能体](#configuring-the-agents)。
3. 点击 **+** 选择预置工作流，或向 **Project Manager** 描述所需的工作流。在画布上检查草稿，然后运行。

如果觉得有用，欢迎[给 rightcommit 点 Star](https://github.com/rightcommit/rightcommit)。
你也可以[报告问题或提出建议](https://github.com/rightcommit/rightcommit/issues)，或阅读[贡献指南](CONTRIBUTING.md)。

## 为什么选择 rightcommit

- **已验证的提交（commit）。** 构建工作流，要求通过测试、智能体审查和人工审批后才能提交。
- **协调多个智能体。** 在同一工作流中，为不同步骤分配 Claude Code、Codex 或 Pi。
- **Token 与成本追踪。** 在 FinOps 中按智能体、模型和执行查看成本与 Token 用量。

## 主要功能

- **智能体工作流编排。** 设计并运行结合 AI 智能体和工具的工作流。工作流可发布为兼容 A2A 的智能体。
- **质量门禁。** 检查任务是否已关闭，运行服务端测试。如果文件在测试后发生变化，则重新验证结果。
- **可追溯的工作记录。** 每个步骤都保存文件版本，记录 SHA-256 指纹和逐行差异。检查失败时，在限定次数内退回修改。
- **按需构建工作流。** 可以只用三个步骤，也可以搭建完整交付流程。可从预置工作流开始，例如 Spec Kit 的规范 → 计划 → 任务 → 实现，也可自行创建。
- **执行控制。** 支持交互、辅助和无人值守模式。可暂停等待人工决策，再从暂停处继续。
- **适用于其他任务。** 同样的模块也可用于研究、文档分析或报告生成工作流。

## 连接与集成

- **智能体：** 支持 Claude Code、Codex 和 Pi。可使用 API 密钥或已获授权的网关，例如 Anthropic、OpenAI、OpenRouter、AWS Bedrock、Google Vertex 和 Microsoft Foundry。
- **服务：** 支持 GitHub、Azure DevOps、ServiceNow，以及通过 Atlassian CLI 接入的 Jira、Confluence 和 Bitbucket。每次向外部系统写入前，都需要确认具体写入内容。
- **开放标准：** 使用 MCP 为智能体接入 MCP 服务器上的工具，使用 A2A 发布可供其他智能体调用的工作流。

## 适用人群

rightcommit 支持个人使用，也支持在组织内部部署并使用独立账户。请阅读[服务提供方条款与许可用途](PROVIDER_TERMS.md)和[预期用途与第三方服务提供方义务](INTENDED_USE.md)。服务提供方名称用于标识集成，不代表存在关联或获得背书。

- **开发者：** 在自己的笔记本电脑上运行智能体，使用自己的密钥。
- **小型团队：** 自托管部署，各用户使用独立的账户、密钥和工作区。

## 其他安装方式

| 指南 | 适用场景 |
|---|---|
| [INSTALL.md](INSTALL.md) | 在 macOS、Windows 和 Linux 上使用 Docker，含分步说明 |
| [INSTALL_MAC.md](INSTALL_MAC.md) | 在 macOS 上无容器安装：Homebrew、crates.io 或源码 |
| [INSTALL_LINUX.md](INSTALL_LINUX.md) | 在 Linux 和 WSL2 上无容器安装：crates.io 或源码 |
| [INSTALL_PODMAN.md](INSTALL_PODMAN.md) | 使用 Podman 替代 Docker |

### 管理 Docker 安装

| 操作 | 命令 |
|---|---|
| 停止应用 | `docker compose down` |
| 获取新版本后更新 | `docker compose up -d --build` |
| 更换端口 | 在 `.env` 文件中设置 `RIGHTCOMMIT_PORT=9090`，然后运行 `docker compose up -d` |

> [!WARNING]
> 请勿运行 `docker compose down -v`。此命令会删除 `rightcommit-data` 卷及其中的所有账户、工作流和密钥。

默认情况下，Docker Compose 将应用绑定到 localhost。

<a name="configuring-the-agents"></a>

## 配置智能体

请先阅读[服务提供方条款与许可用途](PROVIDER_TERMS.md)。三个智能体都支持多用户使用。仍需遵守服务提供方的授权要求和凭据权限范围。

| 指南 | 内容 |
|---|---|
| [HOW_TO_CONFIGURE_CLAUDECODE.md](HOW_TO_CONFIGURE_CLAUDECODE.md) | Claude Code：API 密钥、网关、Bedrock、Vertex、Foundry |
| [HOW_TO_CONFIGURE_CODEX.md](HOW_TO_CONFIGURE_CODEX.md) | Codex：API 密钥和兼容的服务提供方配置 |
| [HOW_TO_CONFIGURE_PI.md](HOW_TO_CONFIGURE_PI.md) | 将 Pi 与 Anthropic 或 OpenAI 搭配使用 |
| [HOW_TO_CONFIGURE_PI_OPENROUTER.md](HOW_TO_CONFIGURE_PI_OPENROUTER.md) | 将 Pi 与 OpenRouter 搭配使用 |
| [HOW_TO_CONFIGURE_PI_LOCAL.md](HOW_TO_CONFIGURE_PI_LOCAL.md) | 在 Pi 中使用本地模型（Ollama、LM Studio、vLLM） |

<a name="documentation"></a>

## 文档

| 文档 | 内容 |
|---|---|
| [CHANGELOG.md](CHANGELOG.md) | 各版本的变更 |
| [ADD_A_SKILL.md](ADD_A_SKILL.md) | 一次为 Claude Code、Codex 和 Pi 添加技能 |
| [ADD-PI-PACKAGES.md](ADD-PI-PACKAGES.md) | 将 pi.dev 上的包添加到 Pi 智能体 |
| [ADD-OPENSPEC.md](ADD-OPENSPEC.md) | 在工作流中使用 OpenSpec |
| [UPDATE-PI-DEFAULT-PACKAGES.md](UPDATE-PI-DEFAULT-PACKAGES.md) | 更新 rightcommit 随附的 Pi 工具（面向维护者） |
| [配置指南](guides/README.md) | 配置访问权限并运行工作流示例，涵盖 ServiceNow（ITSM、目录请求和 Spec Kit）、Atlassian（Jira、Bitbucket 和 Confluence）以及 GitHub（Issue 和 CI） |

<a name="license"></a>

## 许可证

Copyright (c) 2026 Lorenzo Toscano

rightcommit 是开源软件，采用 **[Apache-2.0 许可证](LICENSE)** 发布。

根据许可证条款，你可以使用、修改、自托管和分发代码，包括用于商业目的。完整条款见 [LICENSE](LICENSE)。

上述许可适用于 rightcommit 本身。与之配合使用的第三方产品或服务仍受各自条款和[支持的部署范围](INTENDED_USE.md)约束。

随附图标和 Spec Kit 文件的第三方声明见 [`licenses/`](licenses)。

<a name="contributing"></a>

## 参与贡献

欢迎提交 Pull Request。首个 Pull Request 合并前，需要签署[贡献者许可协议](CLA.md)。机器人会在 Pull Request 下留言，请按提示回复指定句子。只需签署一次。参见 [CONTRIBUTING.md](CONTRIBUTING.md)。

发现问题或有建议？欢迎[创建 Issue](https://github.com/rightcommit/rightcommit/issues)。
欢迎用中文提 Issue。

咨询与反馈请发邮件至 [hello@rightcommit.com](mailto:hello@rightcommit.com)。

---

rightcommit 是 Lorenzo Toscano 以个人身份创建并维护的独立业余项目。他在业余时间使用个人资源开发。该项目与其雇主、雇主的客户或其他组织无关，也未获得这些机构的背书或赞助。项目中的观点与决定均为个人立场。

<sub>Claude Code、Codex、Pi、GitHub、Azure DevOps、ServiceNow、Jira、Confluence、Bitbucket、Spec Kit、Rust、Docker 等名称和商标归各自权利人所有。rightcommit 与这些权利人无关联。</sub>
