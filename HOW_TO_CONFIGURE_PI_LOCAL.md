# Configuring Pi with a local model

Pi can use a model that runs on your own computer, with [Ollama](https://ollama.com),
[LM Studio](https://lmstudio.ai), vLLM or any server with an OpenAI-compatible API. No key and
no cloud costs, but you need a capable machine: agents send long prompts and need **tool
calling**.

The idea: a small `models.json` file points Pi's *OpenAI* provider to your local server, and
in the app you select *OpenAI* with the local model's name. The example uses Ollama and
`qwen3-coder:30b`.

## 1. Start the model

```sh
ollama pull qwen3-coder:30b
OLLAMA_CONTEXT_LENGTH=32768 ollama serve
```

Agents need a long context: `OLLAMA_CONTEXT_LENGTH` raises Ollama's default. If you use the
Ollama app, set the context length in its settings instead.

## 2. Find your server address

| rightcommit runs with | Address |
|---|---|
| Local installation | `http://localhost:11434/v1` |
| Docker Desktop (macOS, Windows) | `http://host.docker.internal:11434/v1` |
| Podman | `http://host.containers.internal:11434/v1` |
| Docker on Linux | `http://host.docker.internal:11434/v1`, after the extra step below |

For LM Studio use port `1234`, for vLLM port `8000`, always with `/v1` at the end.

**Docker on Linux only:** add these lines under `rightcommit:` in `compose.yaml`, and make
Ollama listen on all interfaces (`OLLAMA_HOST=0.0.0.0 ollama serve`; keep port 11434 closed in
your firewall):

```yaml
    extra_hosts:
      - "host.docker.internal:host-gateway"
```

## 3. Create models.json

Save this as `models.json`, with your address and model:

```json
{
  "providers": {
    "openai": {
      "baseUrl": "http://host.docker.internal:11434/v1",
      "api": "openai-completions",
      "apiKey": "local",
      "models": [
        { "id": "qwen3-coder:30b", "name": "Qwen3 Coder 30B", "contextWindow": 32768, "maxTokens": 8192 }
      ]
    }
  }
}
```

`id` must be the exact model name used by the server (`ollama list` shows it).
`contextWindow` should match the context length from step 1.

Then copy it into the account's Pi folder, from the folder that contains `models.json`
(account ID 1 shown here):

**Docker** (with Podman, write `podman` instead of `docker`; run it from the rightcommit
folder, with `models.json` there):

```sh
docker compose exec -T --user 20001:20001 rightcommit sh -c 'cat > /data/users/1/.pi/agent/models.json' < models.json
```

**Local installation:** copy it to `data/users/1/.pi/agent/models.json` inside the folder you
start rightcommit from.

For another account, change `1` to its ID and `20001` to 20000 + the ID.

## 4. Select it in the app

1. Open **Personal settings** → **Agents** → **Pi Agent**.
2. Set **Pi provider** to *OpenAI*.
3. Set **Model** to the model `id`, for example `qwen3-coder:30b`.
4. In **Personal API key** type any text, for example `local`. The local server ignores it;
   it only marks Pi as configured.
5. Press **Save** and run a workflow with Pi.

## Good to know

- **This replaces OpenAI for this account's Pi.** To use the real OpenAI again, delete
  `models.json` from the account's Pi folder.
- **`web_search` does not work** with a local model, because it needs the real OpenAI service.
  `web_fetch` still works.
- **Reasoning:** the **Reasoning** setting has an effect only if you add `"reasoning": true` to
  the model in `models.json` and the model supports it.
- **Small models** often fail at tool calling or long tasks. If Pi stops early or loops, try a
  larger model or a larger context.
- More options (headers, compatibility flags) are in Pi's `models.json` documentation at
  [pi.dev](https://pi.dev).
