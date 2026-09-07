# Contributing

Thanks for taking a look. Skillet is a small Tauri v2 + SvelteKit desktop app; the Rust backend and the Svelte frontend each fit in one sitting.

## Setup

- [Rust](https://rustup.rs/) (stable) and [Node.js](https://nodejs.org/) 20+
- Windows: Microsoft C++ Build Tools and WebView2 (pre-installed on Windows 10/11)
- The [Claude Code](https://claude.com/claude-code) CLI, so `~/.claude` and `claude` exist to scan

```bash
npm install
npx tauri dev                                   # app with hot reload
npm run check                                   # svelte-check
cargo test --manifest-path src-tauri/Cargo.toml # Rust unit tests
npx tauri build                                 # installers in src-tauri/target/release/bundle/
```

## Project structure

```text
src-tauri/src/
|-- lib.rs        # Tauri commands and plugin setup
|-- skills.rs     # skill scanner: personal, project and plugin scopes, SKILL.md frontmatter, description lint
|-- mcp.rs        # `claude mcp list` parser
|-- health.rs     # cross-checks the scanned skills into findings (collisions, weak descriptions, size)
|-- profiles.rs   # loadout profiles in ~/.skillet, built-in templates
|-- launcher.rs   # argv for claude / codex launches
|-- launch.rs     # isolated launch folders, trust gate, terminal start, cleanup
`-- util.rs       # home dir, running the claude CLI, helpers
src/
|-- routes/+page.svelte        # the four views: Skills, MCPs, Agent Starter, Health
|-- routes/+layout.svelte      # window chrome, backdrop, settings pop-out
|-- lib/components/            # SkillCard, McpCard, Settings
|-- lib/api.ts                 # typed wrappers around the Tauri commands
|-- lib/settings.svelte.ts     # persisted settings (theme, motion, backdrop, auto-update)
`-- lib/update.svelte.ts       # updater plugin wiring
```

## Ground rules

- Skillet reads `~/.claude`; it never writes there. Anything Skillet needs to persist goes to `~/.skillet`.
- Parsers (`mcp.rs`, `skills.rs`) mirror what the Claude Code CLI writes today. When you change one, add or update a fixture-based test with real, sanitized output.
- Launch behavior is security-relevant. Keep the trust gate in the Rust backend and keep `runtime/` out of the `--add-dir` root.
- Svelte 5 runes, TypeScript strict, Rust 2021. No new dependencies without a reason in the PR.
- Every network access is documented in `SECURITY.md`. Today that is the update check only.

## Pull requests

- Small, focused changes with a short description of the user-visible effect.
- `npm run check` and `cargo test` before pushing. CI runs both on `windows-latest`.
- Update `CHANGELOG.md` under "Unreleased".

Please follow the [code of conduct](CODE_OF_CONDUCT.md).
