# Cluster — Libra

Libra is the native terminal interface for Cluster, a lightweight personal AI agent runtime. It uses a custom Crossterm renderer, keyboard and mouse navigation, streams provider output, and keeps chat sessions and durable memory separate.

## Providers

Open Settings with `Alt+E` or `/settings`. Choose OpenRouter, OpenAI, llama.cpp, or a locally added OpenAI-compatible endpoint. Provider model catalogues are fetched from the provider's `/models` endpoint when available.

The default local llama.cpp endpoint is `http://127.0.0.1:8080/v1/chat/completions`.

Environment-variable credentials remain supported:

- OpenRouter: `OPENROUTER_API_KEY`
- OpenAI: `OPENAI_API_KEY`
- Custom: `CLUSTER_API_KEY` by default

Credentials entered in Settings are stored in `secrets.enc`, not in the readable configuration file. The optional password lock encrypts that vault with AES-256-GCM and a PBKDF2-HMAC-SHA256-derived key. The password is never stored. There is no recovery key; losing the password means deleting the vault and entering credentials again.

## Interface

- Main menu suggestions are loaded from `~/.config/cluster/suggestions.txt` when present, otherwise from the bundled `data/suggestions.txt` file.
- `Alt+E` opens Settings.
- `Alt+T` switches sessions.
- `Enter` sends a message; `Shift+Enter` or `Ctrl+N` inserts a newline.
- `PageUp` / `PageDown` scroll the conversation.
- `Esc` or `Ctrl+C` exits the current screen.

Settings include provider/model selection, local endpoints, skill and MCP connector metadata, thought-process display visibility, code-depth preview, and password-lock controls. Skill and MCP entries are stored as configuration metadata; they are not yet executed by the agent runtime.

## Commands

- `/help` show commands
- `/settings` open settings
- `/sessions` switch sessions
- `/provider` choose provider and model
- `/models` refresh the current provider's model list
- `/model` show the current model
- `/model <id>` change the current session's model
- `/clear` clear the active session and conversation memory history
- `/memory` show memory status
- `/memory add <text>` save durable memory
- `/memory export` export memory
- `/memory inspect` inspect saved memory
- `/memory reset` reset memory
- `/exit` exit

## Local data

Readable settings are stored at `~/.config/cluster/config.json` (or under `$XDG_CONFIG_HOME`). Existing `config.toml` files are migrated on first load. Encrypted credentials are stored separately at `~/.config/cluster/secrets.enc`.

Conversation memory is stored at `~/.local/share/cluster/memory.json`; independent sessions and their provider/model snapshots are stored at `~/.local/share/cluster/sessions.json` (or under `$XDG_DATA_HOME`).

The current release changelog is fetched from `CHANGELOG.md` in the repository.