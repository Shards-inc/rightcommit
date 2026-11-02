# Configuring Pi

Pi is a multi-provider agent. In rightcommit it can use **Anthropic**, **OpenAI** or
**OpenRouter**, and, with a small file, a **local model**.

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

The key is encrypted in the database and never shown again. Leave the field empty to keep it;
use the bin icon to remove it.

## 2. Pi login

Pi can also sign in with a provider account, for example a Claude or ChatGPT subscription.
Sign in once per account, from a terminal in the rightcommit folder (account ID 1 shown here):

**Docker** (with Podman, write `podman` instead of `docker`):

```sh
docker compose exec -it --user 20001:20001 -e HOME=/data/users/1 -e PI_CODING_AGENT_DIR=/data/users/1/.pi/agent rightcommit pi
```

**Local installation** (from the folder you start rightcommit in):

```sh
export PATH="$PWD/data/system/tools/bin:$PATH"
HOME="$PWD/data/users/1" PI_CODING_AGENT_DIR="$PWD/data/users/1/.pi/agent" pi
```

In Pi type `/login`, pick the provider and follow the steps (paste the final link or code back
when asked), then type `/quit`. Then select the same provider in **Pi provider** in the
app.

> A Pi login **takes priority** over the personal API key. To go back to the key, run Pi again
> as above and use `/logout`.

For another account, change `1` to its ID and `20001` to 20000 + the ID.

## 3. Installation-wide variables

The operator can set keys for the whole installation, used by accounts without a personal key.

- **Docker or Podman:** copy `.env.example` to `.env` (next to `compose.yaml`), fill in only
  what you use, run `chmod 600 .env`, then `docker compose up -d`.
- **Local installation:** `export` the variables in the shell before starting `rightcommit`.

| Pi provider | Variables |
|---|---|
| Anthropic | `ANTHROPIC_API_KEY`, or `ANTHROPIC_OAUTH_TOKEN` / `ANTHROPIC_AUTH_TOKEN` |
| OpenAI | `OPENAI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

`ANTHROPIC_OAUTH_TOKEN` and `ANTHROPIC_AUTH_TOKEN` are used only when the account has no
personal key and `ANTHROPIC_BASE_URL` is empty: tokens meant for a Claude Code gateway are
never sent to Anthropic by Pi.

## Which credential wins

1. The account's Pi login (`/login`).
2. The account's personal API key.
3. The installation-wide variable of the selected provider.

## Tools and options

- Pi gets `read`, `bash`, `edit`, `write`, `grep`, `find`, `ls`, `web_fetch` and `todo`, plus
  `web_search` with Anthropic and OpenAI.
- **Enable codemode** lets the model write small scripts that call several tools at once.
- **Reasoning** is passed to Pi as its thinking level.
- To add more tools, see [ADD-PI-PACKAGES.md](ADD-PI-PACKAGES.md).
