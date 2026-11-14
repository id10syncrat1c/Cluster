# Cluster — Libra

Libra is the native terminal interface for Cluster, a lightweight personal AI agent runtime. It uses a custom Crossterm renderer, keyboard and mouse navigation, streams provider output, and keeps chat sessions and durable memory separate.

## Providers

Open Settings with `Alt+E` or `/settings`. The Providers section separates `Add a provider` from `List connected providers`. OpenRouter and OpenAI are hosted-provider choices. Local models use one `Custom` option with three fields: a display name, a full chat-completions URL, and an optional token. This includes llama.cpp's OpenAI-compatible endpoint; llama.cpp is not a separate provider type. Provider model catalogues are fetched from the endpoint's `/models` route when available. If model discovery is unavailable, the local endpoint uses the `local-model` fallback.

The default local endpoint example is `http://127.0.0.1:8080/v1/chat/completions`.

Environment-variable credentials remain supported for hosted providers:

- OpenRouter: `OPENROUTER_API_KEY`
- OpenAI: `OPENAI_API_KEY`

Credentials entered in Settings are stored in `secrets.enc`, not in the readable configuration file. Token entry is masked; the stored-credentials menu can add, update, or remove credentials for built-in and local providers. The optional password lock encrypts that vault with AES-256-GCM and PBKDF2-HMAC-SHA256 (600,000 iterations). The password is never stored. There is no recovery key; if the password is lost, delete `secrets.enc`. On the next launch Cluster detects the missing vault, disables the lock, and lets you configure credentials again. The lock protects credentials only; conversation memory and session history remain readable JSON files.

## Interface

- Main menu suggestions are loaded from `~/.config/cluster/suggestions.txt` when present, otherwise from the bundled `data/suggestions.txt` file. Suggestions rotate while the home screen is idle.
- Startup creates a fresh session. The home screen offers to resume the previously active session when it has history, or the most recently created session with history otherwise. Choosing No keeps the fresh session active.
- `Alt+E` opens Settings.
- `Alt+T` switches sessions.
- `Enter` sends a message; `Shift+Enter` or `Ctrl+N` inserts a newline.
- `PageUp` / `PageDown` scroll the conversation.
- `Esc` or `Ctrl+C` exits the current screen.

Settings include provider/model selection, local endpoints, skill and MCP connector metadata, thought-process status visibility, code-depth preview, and password-lock controls. Thought-process visibility changes only the UI status display; it does not alter or expose model reasoning. Skill and MCP entries are stored as configuration metadata; they are not yet executed by the agent runtime.

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

Conversation memory is stored at `~/.local/share/cluster/memory.json`; independent sessions and their provider/model snapshots are stored at `~/.local/share/cluster/sessions.json` (or under `$XDG_DATA_HOME`). On Unix-like systems, memory, session, settings, and vault files are written with owner-only permissions (`0600`).

The current release changelog is fetched from `CHANGELOG.md` in the repository.