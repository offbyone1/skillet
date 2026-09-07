# Security

## What Skillet touches

**Reads.** `~/.claude/skills/`, the nearest project `.claude/skills/`, `~/.claude/plugins/installed_plugins.json` and the plugin folders it points to, every `SKILL.md` in those places. It runs `claude --version` and `claude mcp list` from the Claude Code CLI on your `PATH`; `claude mcp list` performs the CLI's own health checks against the MCP servers you configured.

**Writes.** Only its own folder, `~/.skillet`: the loadout profiles and, per Agent Starter launch, a throw-away `launch/<id>/` folder. Skillet never modifies `~/.claude`, project settings or any MCP configuration.

**Network.** Skillet itself makes one kind of request: the update check against `https://github.com/offbyone1/skillet/releases/latest/download/latest.json`, once a day when enabled in Settings, or when you click "Check now". Updates are verified with the minisign public key in `src-tauri/tauri.conf.json` before they install. There is no telemetry and no account. Fonts ship with the app.

## Agent Starter

A loadout launches `claude` or `codex` in a new terminal window with the chosen working directory, model and flags. Two options carry real risk and are shown as such:

- **Headless prompt (`-p`)** runs the agent without an interactive session.
- **`--dangerously-skip-permissions`** lets Claude Code act without its permission prompts.

When either is set and the working directory contains project configuration (`CLAUDE.md`, `.claude/settings.json`, hooks, `.mcp.json`, project skills), Skillet lists what it found and refuses to launch until you confirm that you trust the directory. The check is enforced in the Rust backend, not only in the UI.

For Claude launches Skillet builds an isolated projection: the selected skills are copied into `launch/<id>/skills-root/.claude/skills/` (passed with `--add-dir`), the selected MCP servers are written to `launch/<id>/runtime/mcp.json` (passed with `--mcp-config --strict-mcp-config`) and `--setting-sources project` keeps your user-level skills and settings out of the session. The `runtime/` folder can contain whatever your MCP configuration contains, including headers or environment values. On Unix it is created with mode 0700; on Windows it relies on the ACL of your user profile folder. Launch folders older than 24 hours are removed at the next start.

## What Skillet does not protect against

- Anything the launched agent does inside the working directory you chose. Skillet starts the session; the agent's own permission model applies from then on.
- Skills and MCP servers that are malicious. Skillet inventories them and lints descriptions; it does not sandbox or vet their content.
- Another process running under your Windows account reading `~/.skillet`.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting on this repository ("Security" → "Report a vulnerability"). Do not open a public issue for security problems. You will get an acknowledgement within a few days; fixes ship as a new release with credit unless you prefer otherwise.

## Supported versions

Only the latest release receives fixes.
