# Cluster — Libra

Libra is the CLI-first personal AI agent from Cluster. It supports OpenAI-compatible providers, persistent conversation history, and durable memory.

## Providers

Use `/provider` inside Libra to choose OpenRouter, OpenAI, llama.cpp, or a custom OpenAI-compatible endpoint. Libra attempts to retrieve the provider's `/models` catalogue and lets you select a model. If discovery is unavailable, enter the model ID manually.

API keys are read from environment variables and are never written to the config file:

- OpenRouter: `OPENROUTER_API_KEY`
- OpenAI: `OPENAI_API_KEY`
- Custom: `CLUSTER_API_KEY` by default (configurable during setup)
- llama.cpp: no key by default

The default local llama.cpp endpoint is `http://127.0.0.1:8080/v1/chat/completions`.

## Commands

- `/provider` choose provider and model
- `/models` refresh the current provider's model list
- `/model` show the current model
- `/model <id>` change the model
- `/thinking` toggle the live activity indicator
- `/clear` clear conversation history
- `/memory` show memory status
- `/memory add <text>` save durable memory
- `/memory export` export memory
- `/memory inspect` inspect saved memory
- `/memory reset` reset memory
- `/exit` exit

Configuration is stored at `~/.config/cluster/config.toml` (or under `$XDG_CONFIG_HOME`). Conversation memory remains separate at `~/.local/share/cluster/memory.json` (or under `$XDG_DATA_HOME`).

The live speed gauge is an estimate based on streamed output characters (approximately four characters per token). It is not tokenizer-exact. The activity indicator reports generation activity, not private model reasoning.
