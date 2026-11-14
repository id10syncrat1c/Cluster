# Cluster — Libra changelog

## 0.4.0

- Introduced the native Crossterm terminal interface.
- Added streaming responses for OpenAI-compatible providers.
- Added persistent conversation history and durable memory.
- Added the initial provider catalogue and model discovery flow.

## Unreleased

### Fixed
- License, attribution, and changelog content now open in a standalone document viewer instead of appearing as AI chat messages.
- Assistant labels now appear once per response, with explicit newline and wrapped lines using continuation indentation.
- The TUI rechecks terminal dimensions during redraw so viewport changes from the Termux keyboard are handled even if a resize event is missed.

### Changed
- Provider settings now have separate Add a provider and List connected providers screens. OpenRouter and OpenAI are grouped as hosted providers; llama.cpp and Custom are grouped under Local / custom endpoints.

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
