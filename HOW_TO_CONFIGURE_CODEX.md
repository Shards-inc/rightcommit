# Configuring Codex

rightcommit runs Codex for you. You only need to give it a way to authenticate.
Pick **one** of the options below.

| Option | Who it applies to | Where you set it |
|---|---|---|
| [1. Personal API key](#1-personal-api-key) | your account | in the app |
| [2. ChatGPT login](#2-chatgpt-login) | your account | one terminal command |
| [3. Installation-wide variables](#3-installation-wide-variables) | every account | `.env` file or shell |
| [4. Another provider (advanced)](#4-another-provider-advanced) | your account | Codex `config.toml` |

To check which option is in use, open **Personal settings** → **Environment**: the Codex card
shows the method, the provider, where the credential comes from and whether the login file is
used. It does not check online that the credential is valid.

## 1. Personal API key

The simplest option.

1. Create a key on the [OpenAI platform](https://platform.openai.com/api-keys).
2. In rightcommit open **Personal settings** → **Agents** → **Codex**.
3. Paste the key in **Personal API key**, optionally set a **Model** (empty = the CLI default)
   and press **Save**.

The key is encrypted in the database and never shown again. Leave the field empty to keep it;
use the bin icon to remove it.

## 2. ChatGPT login

Use your ChatGPT plan instead of an API key. Sign in once per account, from a terminal in the
rightcommit folder (account ID 1 shown here):

**Docker** (with Podman, write `podman` instead of `docker`):

```sh
docker compose exec -it --user 20001:20001 -e HOME=/data/users/1 -e CODEX_HOME=/data/users/1/.codex rightcommit codex login --device-auth
```

**Local installation** (from the folder you start rightcommit in):

```sh
export PATH="$PWD/data/system/tools/bin:$PATH"
HOME="$PWD/data/users/1" CODEX_HOME="$PWD/data/users/1/.codex" codex login --device-auth
```

Open the link shown, enter the code and confirm. The login is saved in the account's private
folder (`.codex/auth.json`) and survives restarts.

For another account, change `1` to its ID and `20001` to 20000 + the ID.

## 3. Installation-wide variables

The operator can set credentials for the whole installation. Every account can use them, but
their values are never shown in the app.

- **Docker or Podman:** copy `.env.example` to `.env` (next to `compose.yaml`), fill in only
  what you use, run `chmod 600 .env`, then `docker compose up -d`.
- **Local installation:** `export` the variables in the shell before starting `rightcommit`.

| Variable | Use |
|---|---|
| `CODEX_API_KEY` | API key for Codex |
| `OPENAI_API_KEY` | API key, used when `CODEX_API_KEY` is empty |
| `CODEX_ACCESS_TOKEN` | access token, used when there is no API key |
| `OPENAI_BASE_URL` | an OpenAI-compatible endpoint instead of api.openai.com |

## 4. Another provider (advanced)

Codex reads its own `config.toml` from the account's private folder:
`/data/users/<ID>/.codex/config.toml` with Docker or Podman,
`data/users/<ID>/.codex/config.toml` with a local installation.

There you can select another `model_provider`, for example Amazon Bedrock. When this file
exists, rightcommit also passes the installation's `AWS_*` variables to Codex. Other provider
variables are not passed, and AWS profiles or SSO need their files inside the container.
See the Codex documentation for the file format.

## Which credential wins

rightcommit picks one credential and never falls back to another one silently:

1. A non-OpenAI provider selected in `config.toml`, with its own authentication.
2. The account's personal API key.
3. `CODEX_API_KEY`, then `OPENAI_API_KEY`.
4. `CODEX_ACCESS_TOKEN`.
5. The account's ChatGPT login (`auth.json`).

## Good to know

- **Reasoning:** the **Reasoning** setting is passed to Codex as `model_reasoning_effort`.
- **Changes to `.env`:** run `docker compose up -d` again. `docker compose restart` does not
  reload variables.
- **Proxies:** `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`, `SSL_CERT_FILE` and
  `CODEX_CA_CERTIFICATE` are passed to Codex too. Certificate files must exist inside the
  container.
