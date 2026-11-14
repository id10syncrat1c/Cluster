# Cluster — Libra changelog

## 0.4.0

- Introduced the native Crossterm terminal interface.
- Added streaming responses for OpenAI-compatible providers.
- Added persistent conversation history and durable memory.
- Added the initial provider catalogue and model discovery flow.

## Unreleased

### Fixed
- License, attribution, and changelog content now open in a standalone document viewer instead of appearing as AI chat messages.
- Consecutive assistant output fragments are grouped as one response so only the first displayed line carries the ai label; wrapped and newline-separated content uses continuation indentation.
- The TUI rechecks terminal dimensions and redraws immediately during response streaming when the Termux keyboard changes the viewport.

### Changed
- Provider settings now group OpenRouter and OpenAI as hosted providers, with local models configured through one Custom endpoint form (name, completion URL, optional token). llama.cpp uses this same custom endpoint category.

- Added a last-session resume prompt while keeping startup on a fresh session by default.
- Connected the home screen to rotating external prompt suggestions and clickable changelog navigation.
- Added masked API-token entry, credential add/update actions, and recovery when the encrypted vault is deleted.
- Restricted memory file permissions and fixed incremental compact previews and SSE completion handling.
- Improved the home screen's release branding, shortcut hints, and maintainer credit.

- Added a keyboard- and mouse-driven home menu with external prompt suggestions.
- Added provider and model settings, local OpenAI-compatible endpoints, and per-session provider snapshots.
- Added independent persistent chat sessions and legacy history migration.
- Added readable JSON settings with TOML migration.
- Added an optional password-protected AES-256-GCM credential vault.
- Added skill and MCP connector metadata management.
- Added thought-process visibility and concise/detailed code-edit preview preferences.
- Added a repository-backed changelog view.
