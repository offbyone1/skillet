# Skillet

A desktop atelier for your **Claude Code** setup — see every skill and MCP
server you have installed, judged at a glance.

![MIT License](https://img.shields.io/badge/license-MIT-blue)
![Windows](https://img.shields.io/badge/platform-Windows-blue)
![Tauri v2](https://img.shields.io/badge/Tauri-v2-orange)

## What it does

Skillet scans the skills and MCP servers that the **Claude Code** CLI keeps on
your machine and lays them out in one calm, dark-gold window. Personal,
project, and plugin skills land in a single inventory; each plugin folds into a
neat card stack you can pop open; and a quick description-quality check tells
you which skills will actually trigger. That's the whole product — no account
login, no telemetry, nothing to paste.

This is the **M0** milestone: Skillet reads from disk, it does not write. The
enable/disable and favourite toggles are an interactive preview of what the
upcoming write mode will do.

## Features

- One inventory of every skill — **personal**, **project**, and **plugin**
- Plugin skills collapse into a **card stack**; click to pop out the sub-skills
- **Description-quality** dots (good / weak / poor) so dead skills stand out
- **Favourites** — star a skill to pin it above the rest
- **MCP servers** view — transport, scope, and live connection status
- **Health** view — surfaces conflicts, drift, and weak descriptions
- Card and dense **list** density, with per-skill detail pop-outs
- Custom frameless window chrome and a quiet, design-matched scrollbar
- Everything runs in the Rust backend; no telemetry

## How it works

Skillet never asks you for anything. It reads what the Claude Code CLI already
wrote under `~/.claude`:

**Skills** — three scopes are scanned. Personal skills from
`~/.claude/skills/`, project skills from the nearest `.claude/skills/`, and
plugin skills resolved from `~/.claude/plugins/installed_plugins.json` so each
plugin is counted once (no cache/marketplace duplicates). Every `SKILL.md` is
parsed for its frontmatter, and the description is run through a small lint that
checks whether it signals *when* the skill should fire.

**MCP servers** — pulled from `claude mcp list`, with transport, scope, and
connection status. The call runs on a worker thread with a timeout so a hanging
health check can never freeze the window.

**Health** — the scanned skills and servers are cross-checked for issues
(name collisions, drift on disk, weak descriptions) and listed by severity.

All reads happen in the Rust backend; nothing leaves your machine.

## Requirements

- **Windows 10/11** with WebView2 (pre-installed on modern Windows)
- The [Claude Code](https://claude.com/claude-code) CLI installed, so that
  `~/.claude` and the `claude` command exist

## Build from source

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+
- Windows: Microsoft C++ Build Tools

### Steps

```bash
git clone https://github.com/offbyone1/skillet.git
cd skillet
npm install
npx tauri build      # installers land in src-tauri/target/release/bundle/
```

For development with hot reload:

```bash
npm install
npx tauri dev
```

## Status

M0 — read-only. Skillet scans and displays; it does not modify anything on
disk. Planned next: write mode (actually enable/disable skills and servers) and
**Agent-Starter** loadout profiles that bundle a set of skills + MCPs for a
given working mode.

## License

[MIT](LICENSE)
