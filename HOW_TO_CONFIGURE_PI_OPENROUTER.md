# Configuring Pi with OpenRouter

[OpenRouter](https://openrouter.ai) gives you hundreds of models (Claude, GPT, Gemini, Qwen,
DeepSeek and more) with one key and one bill.

Pi is available to every user. Use an OpenRouter account or API
key you are authorized to use, with billing and model access covered by your agreement.
Read [Provider terms and permitted use](PROVIDER_TERMS.md).

## Steps

1. Create a key on [openrouter.ai/keys](https://openrouter.ai/keys) and add credit to your
   account.
2. In rightcommit open **Personal settings** → **Agents** → **Pi Agent**.
3. Set **Pi provider** to *OpenRouter*. The app loads the OpenRouter model list.
4. In **Model**, start typing to search, for example `claude` or `qwen`, and pick a model.
   IDs look like `vendor/model`. If the list does not load, type the ID by hand (copy it from
   the model page on openrouter.ai).
5. Paste the key in **Personal API key** and press **Save**.

Pick a model that supports **tool calling**: Pi needs tools to read and edit files.

## Other ways to give the key

- **Installation-wide:** set `OPENROUTER_API_KEY` in `.env` (Docker or Podman, then
  `docker compose up -d`) or in the shell before starting `rightcommit`. It is used by accounts
  without a personal key.
- **Administrator key:** an OpenRouter key saved by the administrator in Pi settings is
  used by users without a personal key for that same provider and endpoint.

## Good to know

- **No `web_search`:** OpenRouter has no built-in web search, so Pi does not get the
  `web_search` tool. `web_fetch` still works, so the agent can read pages you name.
- **Limits:** OpenRouter applies the guardrails and provider restrictions set on your key.
- **Costs:** the FinOps panel uses OpenRouter's public prices for its estimates.
