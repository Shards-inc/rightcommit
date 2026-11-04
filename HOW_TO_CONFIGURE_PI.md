# Configuring Pi

Pi is a multi-provider agent. In rightcommit it can use **Anthropic**, **OpenAI** or
**OpenRouter**, and, with a small file, a **local model**. Pi is available to every user. Read [Provider terms and permitted use](PROVIDER_TERMS.md)
before connecting a provider; technical support for a credential is not permission to use it.

| Guide | For |
|---|---|
| this page | Pi with Anthropic or OpenAI |
| [HOW_TO_CONFIGURE_PI_OPENROUTER.md](HOW_TO_CONFIGURE_PI_OPENROUTER.md) | Pi with OpenRouter (hundreds of models with one key) |
| [HOW_TO_CONFIGURE_PI_LOCAL.md](HOW_TO_CONFIGURE_PI_LOCAL.md) | Pi with a model running on your computer (Ollama, LM Studio, vLLM) |

## 1. Personal API key

The simplest option.

1. Create a key: [Claude Console](https://console.anthropic.com/settings/keys) for Anthropic,
   or [OpenAI platform](https://platform.openai.com/api-keys) for OpenAI.
2. In rightcommit open **Personal settings** → **Agents** → **Pi Agent**.
3. Set **Pi provider** to *Anthropic* or *OpenAI*.
4. Paste the key in **Personal API key**, optionally set a **Model** (for example
   `claude-sonnet-5-5`; empty = Pi's default for that provider) and press **Save**.

The key is encrypted in the database; the full value is not returned by the app. Leave the
field empty to keep it; use the bin icon to remove it.

## 2. Environment variables

The operator can set authorized API credentials for the installation, used by accounts
without a personal key. Only permit users covered by the key owner's provider agreement. Use only credentials authorized for these users and workloads.

- **Docker or Podman:** copy `.env.example` to `.env` (next to `compose.yaml`), fill in only
  what you use, run `chmod 600 .env`, then `docker compose up -d`.
- **Local installation:** `export` the variables in the shell before starting `rightcommit`.

| Pi provider | Variables |
|---|---|
| Anthropic | `ANTHROPIC_API_KEY` (API billing) |
| OpenAI | `OPENAI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

## 3. Precedence

1. The user's own API key for the selected provider.
2. The administrator's Pi API key for that same provider and endpoint.
3. Provider-specific credentials from the environment.

An administrator's key is inherited only for the same Pi provider and endpoint, and only
when configured in single-provider mode. It is not sent to a different provider or URL.
In Multi-provider mode the generic personal field is ignored; the administrator's
single-provider key can still cover that matching provider, while other providers require
their own environment configuration. Shared keys are never exposed in another user's settings.

## Tools and options

- Pi gets `read`, `bash`, `edit`, `write`, `grep`, `find`, `ls`, `web_fetch` and `todo`, plus
  `web_search` with Anthropic and OpenAI.
- **Enable codemode** lets the model write small scripts that call several tools at once.
- **Reasoning** is passed to Pi as its thinking level.
- To add more tools, see [ADD-PI-PACKAGES.md](ADD-PI-PACKAGES.md).
