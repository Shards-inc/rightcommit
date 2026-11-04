# Configuring Codex

rightcommit runs the official, unmodified Codex CLI. Supported configuration uses API keys
or authorized provider configuration for individual or internal organizational use.
Read [Provider terms](PROVIDER_TERMS.md) and [Intended use](INTENDED_USE.md).

## Original vendor CLI

rightcommit uses Codex CLI as supplied by OpenAI, installed from the official
`@openai/codex` package at the version pinned by the distribution. rightcommit does
not patch its source code or executables, alter its internal operation, or replace its
native authentication mechanisms or model-access protocols. The CLI itself handles
provider authentication and model requests using its supported interfaces.

rightcommit orchestrates the original CLI through command-line options, environment
variables and supported configuration. The application-level rules below determine which
API key or provider configuration is supplied to each user's process; they do not modify
the CLI's built-in access mechanisms. Provider agreements, permissions and usage limits
continue to apply. This statement is not a certification of overall terms compliance.

## 1. API key in the app

Create a key on the [OpenAI platform](https://platform.openai.com/api-keys).
Open **Personal Settings → Agents → Codex**, enter the API key and save.
The full key is encrypted in the database and is not returned by the app.
Leave the field empty to retain it; use the bin button to remove it.

The administrator's saved Codex API key is also the default for other users who have no
personal Codex key. Each user's own key overrides it. Shared keys are never returned to
other users or copied into their settings.

## 2. Configuration through environment variables

This section applies only to administrator ID 1. Other users do not inherit provider
credentials, cloud/gateway configuration, endpoints or model settings from the server environment.
Their shared fallback is exclusively the administrator's API key saved in Personal Settings.

Edit `.env` for Docker and recreate the service with `docker compose up -d`. Protect the
file with `chmod 600 .env`. For native installations, export variables before startup.

| Variable | Purpose |
|---|---|
| `CODEX_API_KEY` | Organizational API key for Codex |
| `OPENAI_API_KEY` | Used when `CODEX_API_KEY` is empty |
| `OPENAI_BASE_URL` | Authorized OpenAI-compatible endpoint |

Both Compose files forward the declared CLI variables. Custom providers can be selected
through the user's private `.codex/config.toml`; their own provider configuration and
agreement apply. AWS variables are forwarded for native provider configuration when no
app API key is selected. Credential files must be available inside the container.

## 3. Precedence

For the default OpenAI provider:

1. The user's Codex API key in Personal Settings.
2. The administrator's Codex API key in Personal Settings.
3. For the administrator only, `CODEX_API_KEY`, then `OPENAI_API_KEY` from the environment.

An app API key excludes global endpoint overrides. A custom provider explicitly selected
in the user's native configuration uses that provider's authentication; remove that selection
to use the default OpenAI route. See the [official authentication documentation](https://learn.chatgpt.com/docs/auth?surface=cli).

Reasoning is passed as `model_reasoning_effort`. Proxy and certificate variables are listed
in `.env.example`. Recreate the container after environment changes; a plain restart does
not reload `.env`.
