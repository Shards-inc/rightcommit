# Configuring Claude Code

rightcommit runs Claude Code for you. You only need to give it a way to authenticate.
Pick **one** of the options below.

| Option | Who it applies to | Where you set it |
|---|---|---|
| [1. Personal API key](#1-personal-api-key) | your account | in the app |
| [2. Claude subscription login](#2-claude-subscription-login) | your account | one terminal command |
| [3. Installation-wide variables](#3-installation-wide-variables) | every account | `.env` file or shell |

To check which option is in use, open **Personal settings** → **Environment**: the Claude Code
card shows the method, the provider and where the credential comes from. It does not check
online that the credential is valid.

## 1. Personal API key

The simplest option.

1. Create a key in the [Claude Console](https://console.anthropic.com/settings/keys).
2. In rightcommit open **Personal settings** → **Agents** → **Claude Code**.
3. Paste the key in **Personal API key**, optionally set a **Model** (for example `opus` or
   `sonnet`; empty = the CLI default) and press **Save**.

The key is encrypted in the database and never shown again. Leave the field empty to keep it;
use the bin icon to remove it.

## 2. Claude subscription login

Use your Claude Pro or Max plan instead of an API key. Sign in once per account, from a
terminal in the rightcommit folder (account ID 1 shown here):

**Docker** (with Podman, write `podman` instead of `docker`):

```sh
docker compose exec -it --user 20001:20001 -e HOME=/data/users/1 -e CLAUDE_CONFIG_DIR=/data/users/1/.claude rightcommit claude
```

**Local installation** (from the folder you start rightcommit in):

```sh
export PATH="$PWD/data/system/tools/bin:$PATH"
HOME="$PWD/data/users/1" CLAUDE_CONFIG_DIR="$PWD/data/users/1/.claude" claude
```

In Claude Code type `/login`, choose your Claude account, open the link, paste the code back,
then type `/exit`. The login is saved in the account's private folder and survives restarts.

For another account, change `1` to its ID and `20001` to 20000 + the ID.

## 3. Installation-wide variables

The operator can set credentials for the whole installation. Every account can use them, but
their values are never shown in the app.

- **Docker or Podman:** copy `.env.example` to `.env` (next to `compose.yaml`), fill in only
  what you use, run `chmod 600 .env`, then `docker compose up -d`.
- **Local installation:** `export` the variables in the shell before starting `rightcommit`.

| Method | Variables |
|---|---|
| API key | `ANTHROPIC_API_KEY` |
| Subscription token (from `claude setup-token`) | `CLAUDE_CODE_OAUTH_TOKEN` |
| Gateway or proxy | `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN`, or an `Authorization`, `x-api-key` or `api-key` line in `ANTHROPIC_CUSTOM_HEADERS` |
| Amazon Bedrock | `CLAUDE_CODE_USE_BEDROCK=1`, `AWS_REGION` and `AWS_BEARER_TOKEN_BEDROCK` or `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` |
| Bedrock Mantle | `CLAUDE_CODE_USE_MANTLE=1` with AWS credentials as above |
| Claude Platform on AWS | `CLAUDE_CODE_USE_ANTHROPIC_AWS=1` and `ANTHROPIC_AWS_API_KEY` (plus `ANTHROPIC_AWS_WORKSPACE_ID` or `ANTHROPIC_AWS_BASE_URL` if your setup needs them) |
| Google Vertex AI | `CLAUDE_CODE_USE_VERTEX=1`, `CLOUD_ML_REGION`, `ANTHROPIC_VERTEX_PROJECT_ID`, `GOOGLE_APPLICATION_CREDENTIALS` |
| Microsoft Foundry | `CLAUDE_CODE_USE_FOUNDRY=1`, `ANTHROPIC_FOUNDRY_RESOURCE`, `ANTHROPIC_FOUNDRY_API_KEY` (or Azure client credentials) |

The full list of supported variables is in `.env.example`. Other AWS, Google and Azure
variables listed there (profiles, web identity, base URLs) are passed on as well.

`ANTHROPIC_MODEL` and `ANTHROPIC_DEFAULT_OPUS_MODEL` / `_SONNET_` / `_HAIKU_` set default
models; a model saved in the app takes priority.

## Which credential wins

rightcommit picks one credential and never falls back to another one silently:

1. A cloud provider (Bedrock, Mantle, Claude Platform on AWS, Vertex or Foundry). Only one may
   be on, and it applies to **every account**, even those with a personal key.
2. An authentication header in `ANTHROPIC_CUSTOM_HEADERS`.
3. `ANTHROPIC_AUTH_TOKEN`.
4. `CLAUDE_CODE_OAUTH_TOKEN`.
5. The account's personal API key.
6. `ANTHROPIC_API_KEY`.
7. The account's subscription login.

## Good to know

- **Files:** a variable that points to a file (`GOOGLE_APPLICATION_CREDENTIALS`,
  `AWS_SHARED_CREDENTIALS_FILE`, CA certificates) must use a path that exists **inside the
  container**. Mount the file in `compose.yaml`, for example
  `- ./gcp-key.json:/run/secrets/gcp-key.json:ro`, and use `/run/secrets/gcp-key.json`.
- **Changes to `.env`:** run `docker compose up -d` again. `docker compose restart` does not
  reload variables.
- **Proxies:** `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY` and `NODE_EXTRA_CA_CERTS` are passed to
  Claude Code too.
