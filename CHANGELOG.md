# Changelog

All notable changes to Skillet. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] – 2026-09-07

First public release.

### Added
- Skills inventory: personal, project and plugin skills in one view, plugin skills folded into card stacks, description-quality dots, favorites, card and list density, per-skill detail pop-outs.
- MCP servers view from `claude mcp list`: transport, scope and connection status.
- Health view: name collisions across scopes, weak descriptions and oversized skill folders, listed by severity.
- Agent Starter: loadout profiles (working directory, agent, skills, MCP servers, model, effort, headless prompt, skip-permissions) that launch `claude` or `codex` in a terminal. Claude launches are isolated per session; a trust gate in the backend blocks risky launches into directories with project configuration until confirmed. Three built-in templates.
- Settings: dark and light theme, reduced motion, background pattern, automatic update checks.
- Signed in-app updates from GitHub releases (Tauri updater), CI and release workflows, Windows installers (NSIS and MSI).

### Changed
- Fonts ship with the app instead of loading from Google Fonts; the webview has a content security policy.
- App identity: product name, bundle identifier and package names are Skillet's (the scaffold placeholders are gone), with a proper icon.
- UI text is English throughout.

### Removed
- The enable/disable switches on skills and plugins. They only previewed a write mode that does not exist yet; Skillet stays read-only on `~/.claude`.
