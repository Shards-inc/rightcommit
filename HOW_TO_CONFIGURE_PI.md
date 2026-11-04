# Configuring Pi

Pi is a multi-provider agent. rightcommit exposes the providers in its pinned catalog,
including Anthropic, OpenAI, OpenRouter, cloud services and local models. Pi is available
to every user. Read [Provider terms and permitted use](PROVIDER_TERMS.md) before connecting
a provider; technical support for a credential is not permission to use it.

| Guide | For |
|---|---|
| this page | API-key setup, complete provider environment list and precedence |
| [HOW_TO_CONFIGURE_PI_OPENROUTER.md](HOW_TO_CONFIGURE_PI_OPENROUTER.md) | Pi with OpenRouter (hundreds of models with one key) |
| [HOW_TO_CONFIGURE_PI_LOCAL.md](HOW_TO_CONFIGURE_PI_LOCAL.md) | Pi with a model running on your computer (Ollama, LM Studio, vLLM) |

## 1. Personal API key

The simplest option.

1. Create a key: [Claude Console](https://console.anthropic.com/settings/keys) for Anthropic,
   or [OpenAI platform](https://platform.openai.com/api-keys) for OpenAI.
2. In rightcommit open **Personal settings** → **Agents** → **Pi Agent**.
3. Set **Pi provider** to *Anthropic* or *OpenAI*.
4. Paste the key in **Personal API key**, set an explicit **Model** available to that provider
   and press **Save**. Pi requires a model in the app or the Agent node.

The key is encrypted in the database; the full value is not returned by the app. Leave the
field empty to keep it; use the bin icon to remove it.

## 2. Environment variables

The operator can set authorized API credentials for the installation, used by accounts
without a personal key. Only permit users covered by the key owner's provider agreement. Use only credentials authorized for these users and workloads.

- **Docker or Podman:** copy `.env.example` to `.env` (next to `compose.yaml`), fill in only
  what you use, run `chmod 600 .env`, then `docker compose up -d`.
- **Local installation:** `export` the variables in the shell before starting `rightcommit`.

### Complete provider variable list

This list matches the pinned Pi **1.4.0** catalog and rightcommit's actual
forwarding rules: **42 provider entries** and **72 distinct provider variables**.
The source is [distribution/pi-providers.json](distribution/pi-providers.json), with the
application's filters applied. Provider-specific variables are passed only to the selected
Pi provider, including in Multi-provider mode. Environment configuration remains available
to Pi users when no applicable app API key is selected.

The table is an allowlist, not a requirement to set every variable in a row. Configure only
the authentication route and parameters required by the selected provider. Empty variables
are not forwarded. A catalog entry without declared variables does not provide an
environment-based authentication route in this version.

| Provider | Pi provider ID | Supported environment variables |
|---|---|---|
| Amazon Bedrock | `amazon-bedrock` | `AWS_REGION`<br>`AWS_DEFAULT_REGION`<br>`AWS_BEARER_TOKEN_BEDROCK`<br>`AWS_ACCESS_KEY_ID`<br>`AWS_SECRET_ACCESS_KEY`<br>`AWS_SESSION_TOKEN`<br>`AWS_PROFILE`<br>`AWS_CONFIG_FILE`<br>`AWS_SHARED_CREDENTIALS_FILE`<br>`AWS_WEB_IDENTITY_TOKEN_FILE`<br>`AWS_ROLE_ARN`<br>`AWS_ROLE_SESSION_NAME`<br>`AWS_CONTAINER_CREDENTIALS_RELATIVE_URI`<br>`AWS_CONTAINER_CREDENTIALS_FULL_URI`<br>`AWS_CONTAINER_AUTHORIZATION_TOKEN`<br>`AWS_CONTAINER_AUTHORIZATION_TOKEN_FILE` |
| Ant Ling | `ant-ling` | `ANT_LING_API_KEY` |
| Anthropic | `anthropic` | `ANTHROPIC_API_KEY`<br>`ANTHROPIC_FEDERATION_RULE_ID`<br>`ANTHROPIC_ORGANIZATION_ID`<br>`ANTHROPIC_SERVICE_ACCOUNT_ID`<br>`ANTHROPIC_IDENTITY_TOKEN_FILE`<br>`ANTHROPIC_WORKSPACE_ID` |
| Azure OpenAI / Foundry | `azure` | `AZURE_OPENAI_API_KEY`<br>`AZURE_OPENAI_BASE_URL`<br>`AZURE_OPENAI_RESOURCE_NAME`<br>`AZURE_OPENAI_API_VERSION`<br>`AZURE_OPENAI_DEPLOYMENT_NAME_MAP`<br>`AZURE_CLIENT_ID`<br>`AZURE_TENANT_ID`<br>`AZURE_CLIENT_SECRET`<br>`AZURE_FEDERATED_TOKEN_FILE` |
| Baseten | `baseten` | `BASETEN_API_KEY` |
| Cerebras | `cerebras` | `CEREBRAS_API_KEY` |
| Cloudflare AI Gateway | `cloudflare-ai-gateway` | `CLOUDFLARE_API_KEY`<br>`CLOUDFLARE_ACCOUNT_ID`<br>`CLOUDFLARE_GATEWAY_ID` |
| Cloudflare Workers AI | `cloudflare-workers-ai` | `CLOUDFLARE_API_KEY`<br>`CLOUDFLARE_ACCOUNT_ID` |
| DeepSeek | `deepseek` | `DEEPSEEK_API_KEY` |
| Fireworks | `fireworks` | `FIREWORKS_API_KEY` |
| GitHub Copilot | `github-copilot` | `COPILOT_GITHUB_TOKEN` |
| Google Gemini | `google` | `GEMINI_API_KEY` |
| Google Vertex AI | `google-vertex` | `GOOGLE_CLOUD_API_KEY`<br>`GOOGLE_APPLICATION_CREDENTIALS`<br>`GOOGLE_CLOUD_PROJECT`<br>`GCLOUD_PROJECT`<br>`GOOGLE_CLOUD_LOCATION` |
| Groq | `groq` | `GROQ_API_KEY` |
| Hugging Face | `huggingface` | `HF_TOKEN` |
| Kimi For Coding | `kimi-coding` | `KIMI_API_KEY` |
| Meta | `meta` | `META_API_KEY` |
| Minimax | `minimax` | `MINIMAX_API_KEY` |
| Minimax Cn | `minimax-cn` | `MINIMAX_CN_API_KEY` |
| Mistral | `mistral` | `MISTRAL_API_KEY` |
| Moonshotai | `moonshotai` | `MOONSHOT_API_KEY` |
| Moonshotai Cn | `moonshotai-cn` | `MOONSHOT_API_KEY` |
| NVIDIA NIM | `nvidia` | `NVIDIA_API_KEY` |
| OpenAI | `openai` | `OPENAI_API_KEY` |
| OpenAI Codex | `openai-codex` | No provider environment variables declared in this version. |
| Opencode | `opencode` | `OPENCODE_API_KEY` |
| Opencode Go | `opencode-go` | `OPENCODE_API_KEY` |
| OpenRouter | `openrouter` | `OPENROUTER_API_KEY` |
| Qwen Token Plan | `qwen-token-plan` | `QWEN_TOKEN_PLAN_API_KEY` |
| Qwen Token Plan Cn | `qwen-token-plan-cn` | `QWEN_TOKEN_PLAN_CN_API_KEY` |
| Qwen Token Plan Individual | `qwen-token-plan-individual` | `QWEN_TOKEN_PLAN_API_KEY` |
| Radius | `radius` | `RADIUS_API_KEY` |
| Together AI | `together` | `TOGETHER_API_KEY` |
| Vercel AI Gateway | `vercel-ai-gateway` | `AI_GATEWAY_API_KEY` |
| xAI (Grok) | `xai` | `XAI_API_KEY` |
| Xiaomi | `xiaomi` | `XIAOMI_API_KEY` |
| Xiaomi Token Plan Ams | `xiaomi-token-plan-ams` | `XIAOMI_TOKEN_PLAN_AMS_API_KEY` |
| Xiaomi Token Plan Cn | `xiaomi-token-plan-cn` | `XIAOMI_TOKEN_PLAN_CN_API_KEY` |
| Xiaomi Token Plan Sgp | `xiaomi-token-plan-sgp` | `XIAOMI_TOKEN_PLAN_SGP_API_KEY` |
| ZAI Coding Plan | `zai` | `ZAI_API_KEY` |
| Zai Coding Cn | `zai-coding-cn` | `ZAI_CODING_CN_API_KEY` |
| Local / OpenAI compatible | `local` | `PI_LOCAL_API_KEY`<br>`PI_LOCAL_BASE_URL` |

### Provider configuration notes

- **Amazon Bedrock:** choose the credential route authorized for your environment; region,
  access-key/session, profile, web-identity and container-credential settings are alternatives
  or supporting parameters, not a single set of mandatory fields.
- **Azure OpenAI / Foundry:** configure the endpoint/resource and the credentials required
  by your selected service. Version, deployment-map and identity parameters depend on that service.
- **Google Vertex AI:** project/location parameters and the selected credentials must match
  the authorized Google Cloud environment.
- **Cloudflare:** account and gateway identifiers accompany the applicable API credentials.
- **Local / OpenAI compatible:** `PI_LOCAL_BASE_URL` supplies the endpoint when no URL is saved
  in the app or node. `PI_LOCAL_API_KEY` is optional when the server requires no authentication.
  Set the exact model name; see [the local model guide](HOW_TO_CONFIGURE_PI_LOCAL.md).

Credential/profile/certificate file paths must be readable **inside the container** when
using Docker or Podman. Host paths are not mounted automatically. Both Compose files forward
the declared provider variables; adding an arbitrary variable to `.env` does not make it
available in the container or in a Pi process. CLI-specific settings from the Claude Code
or Codex guides are not automatically Pi settings.

### Shared proxy and certificate variables

These infrastructure variables are forwarded to Pi independently of the provider:

`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`, `NODE_EXTRA_CA_CERTS`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `CODEX_CA_CERTIFICATE`.

They configure network transport and certificate handling; they do not supply provider API
access. Certificate paths must exist inside the container. See [.env.example](.env.example)
for configuration placeholders. Recreate the container after changing `.env`; a plain
`docker compose restart` does not reload the environment.

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
