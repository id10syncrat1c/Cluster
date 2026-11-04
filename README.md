# Cluster — Libra

Libra is the native terminal interface for Cluster, a lightweight personal AI agent runtime. It uses a custom cell-diff renderer with Crossterm input, streams provider output directly into the UI, and keeps conversation history and durable memory separate from the interface.

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
- `/clear` clear conversation history
- `/memory` show memory status
- `/memory add <text>` save durable memory
- `/memory export` export memory
- `/memory inspect` inspect saved memory
- `/memory reset` reset memory
- `/exit` exit

Configuration is stored at `~/.config/cluster/config.toml` (or under `$XDG_CONFIG_HOME`). Conversation memory remains separate at `~/.local/share/cluster/memory.json` (or under `$XDG_DATA_HOME`).

The terminal UI supports multiline input with Shift+Enter, message scrolling with PageUp/PageDown, and Ctrl+C or Esc to exit. Completion speed is estimated from output characters (approximately four characters per token), so it is not tokenizer-exact.
