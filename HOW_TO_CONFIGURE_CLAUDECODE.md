# Configuring Claude Code

rightcommit runs the official, unmodified Claude Code CLI. The documented configuration
uses API keys or authorized gateway/cloud credentials for individual or internal organizational
use. Read [Provider terms](PROVIDER_TERMS.md) and [Intended use](INTENDED_USE.md).

## Original vendor CLI

rightcommit uses Claude Code as supplied by Anthropic, installed from the official
`@anthropic-ai/claude-code` package at the version pinned by the distribution. rightcommit does
not patch its source code or executables, alter its internal operation, or replace its
native authentication mechanisms or model-access protocols. The CLI itself handles
provider authentication and model requests using its supported interfaces.

rightcommit orchestrates the original CLI through command-line options, environment
variables and supported configuration. The application-level rules below determine which
API key or provider configuration is supplied to each user's process; they do not modify
the CLI's built-in access mechanisms. Provider agreements, permissions and usage limits
continue to apply. This statement is not a certification of overall terms compliance.

## 1. API key in the app

Create an API key in the [Claude Console](https://console.anthropic.com/settings/keys).
Open **Personal Settings → Agents → Claude Code**, enter the API key and save.
The full key is encrypted in the database and is not returned by the app.
Leave the field empty to keep a saved key; use the bin button to remove it.

With one administrator, that key is used by their Claude Code processes. With multiple
users, it is also the default for users without their own Claude Code key. A user's own
key takes precedence. The shared key is never displayed or copied into another user's settings.

## 2. Configuration through environment variables

This section applies only to administrator ID 1. Other users do not inherit provider
credentials, cloud/gateway configuration, endpoints or model settings from the server environment.
Their shared fallback is exclusively the administrator's API key saved in Personal Settings.

For Docker, edit `.env` beside the Compose file, protect it with `chmod 600 .env`, and run
`docker compose up -d`. For native installations, set the variables before starting the app.
Use credentials authorized for the organization and intended users.

| Method | Variables |
|---|---|
| API key | `ANTHROPIC_API_KEY` |
| Gateway or proxy | `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN`, or an `Authorization`, `x-api-key` or `api-key` line in `ANTHROPIC_CUSTOM_HEADERS` |
| Amazon Bedrock | `CLAUDE_CODE_USE_BEDROCK=1`, `AWS_REGION` and `AWS_BEARER_TOKEN_BEDROCK` or `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` |
| Bedrock Mantle | `CLAUDE_CODE_USE_MANTLE=1` with AWS credentials as above |
| Claude Platform on AWS | `CLAUDE_CODE_USE_ANTHROPIC_AWS=1` and `ANTHROPIC_AWS_API_KEY` (plus `ANTHROPIC_AWS_WORKSPACE_ID` or `ANTHROPIC_AWS_BASE_URL` if your setup needs them) |
| Google Vertex AI | `CLAUDE_CODE_USE_VERTEX=1`, `CLOUD_ML_REGION`, `ANTHROPIC_VERTEX_PROJECT_ID`, `GOOGLE_APPLICATION_CREDENTIALS` |
| Microsoft Foundry | `CLAUDE_CODE_USE_FOUNDRY=1`, `ANTHROPIC_FOUNDRY_RESOURCE`, `ANTHROPIC_FOUNDRY_API_KEY` (or Azure client credentials) |


Both Compose files forward the declared CLI variables. File paths for cloud credentials
and certificates must exist and be readable inside the container. Additional supported
cloud and model variables are listed in `.env.example`.

## 3. Precedence

1. The user's API key in Personal Settings.
2. The administrator's Claude Code API key in Personal Settings.
3. For the administrator only, environment configuration: one enabled cloud provider, gateway authentication, or
   `ANTHROPIC_API_KEY` as appropriate for the selected route.

An app API key selects direct Anthropic API billing and excludes global cloud/gateway
credentials and endpoint overrides. The administrator can remove the app key to use environment-based cloud or gateway
configuration. Model defaults remain configurable in the app and environment.
Provider permissions, usage policies and billing remain the credential owner's responsibility.

Changing `.env` requires recreating the container; `docker compose restart` does not reload it.
