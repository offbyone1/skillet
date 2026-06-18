# Agent-Starter — Design-Spec

**Datum:** 2026-06-18
**Status:** Entwurf zur Review
**Projekt:** Skillet (Tauri + SvelteKit Desktop-App zur Verwaltung von Claude-Code Skills & MCPs)

## 1. Überblick

Der **Agent-Starter** ersetzt den bisherigen M3-Placeholder im `starter`-View. Er lässt
den Nutzer ein **Loadout-Profil** definieren — Arbeitsverzeichnis, Agent-CLI, Skill-Set,
MCP-Set und Start-Flags — und damit per Klick eine echte Agent-Session in einem Terminal
starten. Mitgelieferte Templates dienen als Startpunkt; eigene Loadouts werden in Skillets
eigenem App-Data gespeichert.

Kernidee: **Skillet ist die Single Source of Truth (SSOT) für die Skill-/MCP-Bibliothek.**
Jeder Launch erzeugt eine *isolierte, exakte Projektion* der gewählten Skills/MCPs für genau
diese Session — `~/.claude` wird nicht mutiert, parallele Agent-Sessions beeinflussen sich
nicht.

## 2. Ziele / Nicht-Ziele

**Ziele (v1):**
- Loadout-Profile anlegen, bearbeiten, löschen, speichern (Skillet-eigener Speicher).
- Mitgelieferte Templates (builtin, read-only, duplizierbar) + eigene Loadouts.
- Agent `claude` mit *exaktem* Skill-Subset + *exaktem* MCP-Subset isoliert starten.
- Agent `codex` schlank starten (workdir, model, prompt, skip-perms).
- Start-Flags pro Profil: `model`, `effort`, headless-`prompt` (`-p`), `skipPerms`-Toggle.
- Working dir pro Profil als Default, beim Launch überschreibbar (Ordner-Picker).
- Plattform-Terminal automatisch wählen (Windows `wt.exe`, macOS `Terminal`, Linux generisch).

**Nicht-Ziele (v1):**
- Kein Schreiben in `~/.claude` oder Projekt-Config (außer ephemeren, Skillet-eigenen
  Launch-Verzeichnissen).
- Kein eingebettetes Terminal-Panel (xterm.js/pty) — kommt evtl. später.
- Kein Skill-/MCP-Isolations-Mapping für Codex (Codex hat ein eigenes Plugin-/MCP-System).
- Kein Freitext-Extra-Args-Feld (bewusst weggelassen, hält das Profil bounded).
- Keine Live-Verwaltung laufender Sessions (Start ist „fire-and-forget" ins Terminal).

## 3. Getroffene Entscheidungen (mit Begründung)

| Frage | Entscheidung | Begründung |
|------|--------------|-----------|
| Was tut „Start"? | CLI im Terminal launchen | Nutzer will echte Sessions, inkl. `-p` und `--dangerously-skip-permissions`. |
| Agents | `claude` + `codex` | Vom Nutzer gewählt. |
| Vorgefertigte Agents | Templates **und** eigene Loadouts | Beides gewählt. |
| Speicherort | Skillet-eigenes App-Data | Nutzer: „nur Profile in Skillet speichern", kein Config-Write. |
| Terminal | Plattform-Default, auto-detect | `wt.exe` Win, `Terminal` mac, generisch Linux; nicht konfigurierbar in v1. |
| Working dir | Default im Profil, beim Launch überschreibbar | Flexibelster Fall. |
| Profil-Flags | model, effort, `-p` prompt, skipPerms | Vom Nutzer gewählt; kein Freitext-args. |
| SSOT-Modell | **B: launch-scoped Isolation** | Keine Cross-Session-Races, `~/.claude` unberührt. |
| Skill-Subset | **Hart erzwungen** (verifiziert) | Mechanik experimentell bestätigt, siehe §4.1. |

## 4. Launch-Mechanik

### 4.1 Claude — isolierte Projektion (verifiziert, mit Einschränkung)

Belegt durch ein deterministisches Experiment (stream-json `init`-Event, Feld `skills`):

```
# baseline (cwd=neutral):            skills = 91 slash_commands inkl. persönlicher + Plugin-Skills
# + --setting-sources project,local: 30  (persönliche/Plugin weg, gebündelte bleiben)
# + --settings disableBundledSkills:  skills field = ["skilletprobe"]  ← injiziertes Subset
```

**Wichtige Einschränkung (verifiziert):** `--add-dir`-Skills werden als **project**-Scope
behandelt. Folge:
- Mit `project` in `--setting-sources` lädt der **Ziel-workdir seine eigenen
  `.claude/skills`** mit. Test: cwd=workdir-mit-`wdskill` + `--add-dir` mit `addskill` →
  `skills = ["wdskill","addskill"]` (mit `project` **und** mit `project,local`).
- Mit `--setting-sources local` allein (kein `project`) lädt `--add-dir` **gar keine** Skills →
  `skills = []`. `project` ist also für die Injektion zwingend.

**Entscheidung:** `--setting-sources` = **`project`** (kein `local`). Verifiziert: `project`
allein reicht, damit `--add-dir`-Skills laden, und es lässt die **local**-Quelle weg
(gitignored `.claude/settings.local.json` + local-scoped MCP) → kleinere Angriffsfläche.

Skillet kontrolliert damit **exakt** die *persönlichen, Plugin- und gebündelten* Skills
(= das globale Rauschen). Die **projekteigenen Skills des Ziel-workdir laden bewusst immer
mit** — gewollt (wer ein Projekt öffnet, soll dessen Skills haben). „Exaktes Subset" gilt
also relativ zu „global", **nicht** absolut. Siehe §4.2 für die daraus folgende
Trust-Boundary.

Launch-Kommando:

```
cwd = <workdir>
claude \
  --setting-sources project \
  --add-dir <run>/skills-root \                      # enthält <run>/skills-root/.claude/skills/<gewählte>
  --settings '{"disableBundledSkills":true}' \       # + ggf. weitere Settings, s.u.
  --mcp-config <run>/runtime/mcp.json \              # NICHT unter skills-root (s. Trust-Boundary)
  --strict-mcp-config \                              # exaktes MCP-Subset, ignoriert alle anderen
  [--model <alias>] [--effort <low|medium|high|xhigh|max>] \
  [-p "<prompt>"] \                                  # nur wenn prompt gesetzt (headless)
  [--dangerously-skip-permissions]                   # nur wenn skipPerms an
```

**Warum das funktioniert:**
- `--setting-sources project` lässt die **user**-Quelle (und `local`) weg → persönliche
  (`~/.claude/skills`) und user-Plugin-Skills werden nicht geladen.
- `--add-dir <run>/skills-root` lädt Skills aus `<run>/skills-root/.claude/skills/` **nur für
  diese Session** → injiziert das gewählte Subset (zzgl. workdir-Projekt-Skills, §4.2).
- `--settings '{"disableBundledSkills":true}'` entfernt Claudes eingebaute Skills.
- `--mcp-config … --strict-mcp-config` → nur die gewählten MCP-Server, ignoriert alle
  anderen Konfigurationen.

**Launch-dir-Aufbau** (pro Start frisch, in Skillet-App-Data) — **getrennte Bereiche**, weil
`--add-dir` dem Agent *Datei-Tool-Zugriff* auf alles darunter gibt:
```
<appData>/skillet/launch/<run-id>/
  skills-root/.claude/skills/<skill>/...   ← NUR das geht in --add-dir
  runtime/mcp.json                         ← Server-Specs (evtl. Secrets) — NICHT agent-lesbar
  runtime/launcher.(ps1|command|sh)        ← Prompt + argv — NICHT agent-lesbar
  runtime/.lock                            ← Heartbeat für Cleanup (§9)
```
`runtime/` liegt **außerhalb** der `--add-dir`-Wurzel und bekommt owner-only-Rechte
(Windows ACL / unix 0600), damit der gestartete Agent seine eigenen `mcp.json`-Headers/Env
und den Prompt **nicht** über Datei-Tools lesen kann. Wo möglich werden MCP-Secrets als
Env-/Keychain-/OAuth-Referenz statt als Klartext in `mcp.json` abgelegt.

**Skill-Kopie (gehärtet):** Skills werden aus der Master-Bibliothek (real installierte Skills,
`scan_all()` → Feld `path`) **kopiert** (nicht symlinkt — Windows-Symlinks brauchen erhöhte
Rechte). Der Kopierer: **folgt keinen** Symlinks/Junctions (lehnt sie ab), nur das gewählte
kanonische Skill-Verzeichnis, mit Größen-/Datei-/Tiefen-Caps (analog `dir_stats_depth`).

**`mcp.json`-Format:** `{ "mcpServers": { "<name>": { …serverspec… } } }`.

**Quelle der Serverspec (korrigiert):** `claude mcp get <name>` liefert **kein JSON** (nur
Menschen-Text) → ungeeignet. `list_mcp` liefert nur Metadaten **und ist an Skillets eigenes
cwd gebunden**, nicht an den Profil-workdir. MCP-Discovery muss **workdir-bezogen** sein:
den Ziel-Projekt-Root kanonisieren und alle Scopes für *diesen* Root lesen:
- **user-scope:** `~/.claude.json` → top-level `mcpServers`.
- **local-scope:** `~/.claude.json` → `projects[<canonical-workdir>].mcpServers`.
- **project-scope:** `<workdir>/.mcp.json`.
- **Plugin-/managed-Server** (`plugin:*`, `claude.ai *`): Spec lebt im Plugin / ist managed →
  **nicht für `--mcp-config` rekonstruierbar**. v1: im isolierten Modus **hart deaktivieren**
  (nicht wählbar) — Durchreichen würde `--strict-mcp-config` aufweichen und das exakte Subset
  brechen. Ein späterer „non-isolated"-Modus könnte sie explizit als nicht-isoliert erlauben.

**Server-Identität:** Auswahl wird als ID `{scope, name, sourcePath}` gespeichert, **nicht** nur
als Name — Namen kollidieren über Scopes hinweg (user vs. local vs. project).

### 4.2 Trust-Boundary & Sicherheit (CRIT)

`--setting-sources project` lässt zwar die **user**-Quelle weg (persönliche Skills, user-
`settings.json`, user-`CLAUDE.md`, user-Plugins laden nicht), lädt aber die **project**-Quelle
des Ziel-workdir **vollständig** — und das ist **mehr als nur Skills**: project-`settings.json`
(inkl. **Hooks**), `CLAUDE.md`/`CLAUDE.local.md`, project-Plugins, Permissions.

**Konsequenz:** Ein vom User gewählter, *nicht vertrauenswürdiger* workdir ist eine echte
**Ausführungs-/Prompt-Injection-Grenze** — besonders mit `-p` (überspringt den Workspace-
Trust-Dialog) und/oder `--dangerously-skip-permissions`. Ein bösartiger Projekt-Hook oder
eine `CLAUDE.md` könnte beim Start Code ausführen.

**Pflicht-Mitigationen (v1):**
- **Harter Trust-Gate vor dem Launch:** Skillet macht einen Preflight-Scan des workdir auf
  `.claude/settings*.json`, Hooks, `CLAUDE*.md`, `.mcp.json`, project-Plugins und zeigt
  gefundene aktive Elemente an. Launch mit `-p` **oder** `skipPerms` erfordert eine
  **explizite Bestätigung** („diesem Verzeichnis vertrauen").
- **Wording:** überall „clean/isoliert" → **„globales Skill-Rauschen kontrolliert;
  Projekt-Config wird vertraut"**. Skillet isoliert *globalen* Kram, nicht das Projekt selbst.

Optionales Durchreichen einzelner user-Settings (z.B. Permissions) in die Isolation ist
v2-Scope (zusätzliche Schlüssel im `--settings`-JSON).

### 4.3 Codex (schlank, v1)

Flags verifiziert gegen Codex v0.140.0 (`-C/--cd <DIR>`, `codex exec [PROMPT]`,
`--dangerously-bypass-approvals-and-sandbox`):
```
# interaktiv (prompt leer):
codex -C <workdir> [-m <model>] [--dangerously-bypass-approvals-and-sandbox] ["<seed-prompt>"]

# headless (prompt gesetzt):
codex exec -C <workdir> [-m <model>] [--dangerously-bypass-approvals-and-sandbox] "<prompt>"
```
**`codex exec` ist ein One-Shot** (läuft durch, gibt Ergebnis aus, beendet sich) — keine
Session. Wie bei Claude `-p` schließt sich ein reines Launch-Terminal nach Fertigstellung;
v1 nutzt einen Keep-open-Wrapper (§4.4), damit der Output sichtbar bleibt.

**Skills und MCP-Isolation gelten für Codex NICHT** — Codex hat ein eigenes Plugin-/MCP-System
(`codex plugin`, `codex mcp`). Im UI werden Skill-/MCP-/Effort-Felder für `agent=codex`
ausgegraut, mit Hinweis „nur für Claude". `effort` ist Claude-spezifisch.

### 4.4 Terminal-Spawn (plattformabhängig) — injektionssicher

**Sicherheitsprinzip:** Das vollständige Agent-Kommando (mit Prompt, Pfaden, Skill-Namen) wird
**niemals** als Shell-String zusammengebaut, der dann interpoliert wird. Grund: `wt.exe` nutzt
`;` als Tab-/Befehls-Delimiter, `osascript do script "…"` und `powershell -Command "…"` sind
Shell-Strings → ein Prompt mit `;`, `"`, `$()`, Backticks würde das Kommando brechen oder
**Code injizieren** (Prompt ist Freitext = Angriffsfläche).

**Lösung — Launcher-Skript pro Launch:** Skillet schreibt ein kleines Skript in die (eigene)
Launch-dir und lässt das Terminal nur *dieses Skript* ausführen. Das Skript enthält die
gewünschte `cd` + Agent-argv als Array/getrennte Tokens, von Skillet korrekt escaped — keine
Re-Interpolation von User-Input durch das Terminal.
- **Windows (verifiziert):** Launcher = `.ps1`. `ExecutionPolicy` ist hier
  `CurrentUser=RemoteSigned` → ein unsigniertes `.ps1` wird **blockiert**. Daher Start via
  `wt.exe -d <workdir> -- powershell -NoProfile -ExecutionPolicy Bypass -NoExit -File <launcher.ps1>`
  (Fallback ohne `wt.exe`: dasselbe ohne `wt.exe -d … --`). Im `.ps1`: `Set-Location -LiteralPath`
  + Agent via `& <exe> @args` (PowerShell-Array, kein `-Command`-String).
- **macOS:** Launcher = `.command` (chmod +x), Inhalt mit korrekt gequoteten argv. Start via
  `open -a Terminal <launcher.command>` (kein `osascript do script`-String aus User-Input).
- **Linux:** Launcher = `.sh`. Start über `$TERMINAL` → Fallbacks (`x-terminal-emulator`,
  `gnome-terminal`, `konsole`, `xterm`), die das Skript ausführen.

**Executable-Resolution (Windows, verifiziert):** `claude` → `claude.exe` (nativ, ok). `codex`
→ **`codex.ps1`** (ExternalScript, von `RemoteSigned` blockiert), aber **`codex.cmd` existiert**.
Skillet löst die Agent-Executable **explizit** auf (`codex.cmd` bzw. native exe), **nicht** über
PATH/PATHEXT-Reihenfolge oder den `.ps1`-Shim.

**Argv-Encoder (definiert):** Der Rust-`launch_agent`-Command baut argv strikt als getrennte
Tokens (`Vec<String>`) und schreibt sie über einen **definierten Encoder** ins Launcher-Skript —
nicht als Ad-hoc-String-Literale: pro Plattform ein PowerShell-Array bzw. POSIX-`"$@"`-Array,
Tokens einzeln gequotet/escaped (PS: `'…'` mit verdoppelten `'`; POSIX: single-quote-escape).
Inline-`--settings`-JSON, Prompt und Pfade gehen so unverändert und uninterpretiert durch.

**Keep-open:** Bei headless (`-p` / `codex exec`) endet der Agent → das Skript hält das Fenster
offen (z.B. `Read-Host` / `read -n1` am Ende), damit der Output lesbar bleibt. Interaktive
Launches brauchen das nicht. Ein „Hintergrund + Output-capture im UI"-Modus ist v2-Scope.

## 5. Datenmodell

`Loadout`-Profil, gespeichert als JSON-Array in `<appData>/skillet/profiles.json`:

```ts
interface Loadout {
  id: string;            // uuid
  name: string;
  note: string;          // kurze Beschreibung
  agent: "claude" | "codex";
  workdir: string;       // Default-Arbeitsverzeichnis (beim Launch überschreibbar)
  skills: SkillRef[];    // stabile IDs (claude only) — NICHT nur Namen (Scope-Kollisionen)
  mcps: McpRef[];        // stabile IDs (claude only) — NICHT nur Namen
  model: string | null;  // alias (opus/sonnet/...) bzw. codex -m
  effort: string | null; // low|medium|high|xhigh|max (claude only)
  prompt: string | null; // -p headless; null/leer = interaktiv
  skipPerms: boolean;    // --dangerously-skip-permissions
  builtin: boolean;      // true = mitgeliefertes Template (read-only, „Duplizieren" zum Anpassen)
}

// Stabile Referenzen statt bloßer Namen — Namen kollidieren über Scopes/Plugins hinweg.
interface SkillRef { name: string; scope: "personal"|"project"|"plugin"; plugin: string|null; path: string; }
interface McpRef   { name: string; scope: "user"|"local"|"project"|"plugin"|"managed"; sourcePath: string; }
```
Anzeigenamen bleiben kosmetisch; aufgelöst/gematcht wird über die ID (Pfad/Scope/Plugin).
Bei Resolve-Fehlern (Skill/Server inzwischen weg/verschoben) zeigt das UI eine klare Warnung,
statt still etwas Falsches zu starten.

Built-in Templates werden als Konstanten im Code geliefert (nicht in `profiles.json`
gespeichert) und beim `list_profiles` vor die gespeicherten Loadouts gemerged.

**Mitgelieferte Templates (Vorschlag, finale Sets aus echter Skill-/MCP-Liste):**
- **Research** — `deep-research`, `consult-codex`; MCP `github`, `microsoft-learn`.
- **Frontend** — `design-taste-frontend`, `frontend-design`; MCP `playwright`.
- **Debug** — `systematic-debugging`, `bugfix`; agent `codex` als Second-Opinion-Variante.

## 6. Backend — neue Tauri-Commands (Rust)

| Command | Signatur | Zweck |
|--------|----------|------|
| `list_profiles` | `() -> Vec<Loadout>` | Built-in Templates + gespeicherte aus `profiles.json`. |
| `save_profile` | `(p: Loadout) -> Result<()>` | Validieren + nach `profiles.json` schreiben (kein builtin). |
| `delete_profile` | `(id: String) -> Result<()>` | Aus `profiles.json` entfernen. |
| `launch_agent` | `(id: String, workdirOverride: Option<String>) -> Result<()>` | Launch-dir bauen, Skills kopieren, `mcp.json` schreiben, argv assemblieren, Terminal spawnen. |

`launch_agent`-Ablauf:
1. Profil auflösen (builtin oder gespeichert); Skill-/MCP-`Ref`s auflösen (Resolve-Fehler →
   klare Fehlermeldung statt stillem Fallback).
2. workdir = override ?? profile.workdir; existiert? sonst Fehler. **Trust-Gate** (§4.2): bei
   `-p` oder `skipPerms` Preflight-Scan + explizite Bestätigung erforderlich.
3. `agent=claude`: Launch-dir `<appData>/skillet/launch/<run-id>/` mit getrennten Bereichen
   anlegen (`skills-root/` + `runtime/`, §4.1): gewählte Skills gehärtet nach
   `skills-root/.claude/skills/` kopieren, `runtime/mcp.json` (owner-only) aus den workdir-
   bezogenen MCP-Specs schreiben, Launcher + argv nach §4.1/§4.4 bauen.
   `agent=codex`: Launcher + argv nach §4.3/§4.4 bauen.
4. Plattform-Terminal mit cwd=workdir spawnen (§4.4); Launcher schreibt `runtime/.lock` und
   entfernt es beim Exit (Heartbeat).
5. Launch-dir-Cleanup: **nur beim App-Start**, und nur Dirs **ohne aktive `.lock`** die älter
   als ein TTL sind (die letzten N behalten). Nie während/direkt nach einem Launch löschen —
   der File-Watcher der Session kann sie nachladen (§9).

Neue Module: `profiles.rs` (Persistenz + Templates), `launcher.rs` (dir-build, argv, spawn).
Registrierung in `lib.rs` `invoke_handler`.

## 7. Frontend (SvelteKit)

**`src/lib/types.ts`:** `Loadout`-Interface ergänzen.
**`src/lib/api.ts`:** `listProfiles`, `saveProfile`, `deleteProfile`, `launchAgent`,
plus Ordner-Picker (Tauri-dialog-Plugin) Wrapper.
**`src/routes/+page.svelte`** (`starter`-View): Placeholder ersetzen durch
- Template-Galerie (builtin-Karten, „Duplizieren"-Aktion).
- „Meine Loadouts" (gespeicherte Karten: Launch / Bearbeiten / Löschen).
- „+ Neu"-Karte → Edit-Modal.
- **Edit-Modal:** Name, Note, Agent-Dropdown, workdir-Picker, Skill-Multiselect (aus
  gescannten Skills), MCP-Multiselect (aus `listMcp`), Model, Effort, Prompt-Textarea,
  skipPerms-Toggle. Felder werden für `agent=codex` korrekt ausgegraut (§4.3).
- **Launch-Flow:** Launch-Button → optionaler workdir-Override-Dialog → `launchAgent`.

Visuelles Vorbild: `profile-grid` / `profile-card` aus `mockups/mockup-v2-gold.html`.
Stil: bestehendes gold-on-dark v2-gold (Sidebar + Pfannen-Logo).

## 8. Tauri-Konfiguration

- **dialog-Plugin** hinzufügen (`tauri-plugin-dialog`) für den Ordner-Picker; Capability in
  `src-tauri/capabilities/default.json` freischalten.
- **Prozess-Spawn:** via `std::process::Command` im Rust-Command — kein zusätzliches
  shell-Plugin nötig.
- **`runtime/`-Rechte:** owner-only setzen (Windows-ACL bzw. unix `0600`/`0700`) für
  `mcp.json` + Launcher — verhindert Lesen durch den gestarteten Agent (§4.1/§4.2).
- `Cargo.toml`: `tauri-plugin-dialog` als Dependency; serde-Strukturen für `Loadout`/`*Ref`.

## 9. Risiken & offene Punkte

**Entschieden / aufgelöst (nach 2 Review-Runden inkl. Codex):**
- **CRIT — Trust-Boundary:** `--setting-sources project` lädt project-Hooks/`CLAUDE.md`/
  Permissions, nicht nur Skills → untrusted workdir + `-p`/skipPerms = Ausführungsgrenze. Mit
  hartem Trust-Gate + Preflight-Scan mitigiert; Wording „isoliert"→„Projekt wird vertraut"
  (§4.2).
- **HIGH — Secrets/Datei-Zugriff im Launch-dir:** `--add-dir` gibt Tool-Zugriff → `mcp.json`/
  Launcher (Prompt, Headers, Env) müssen **außerhalb** der add-dir-Wurzel + owner-only liegen.
  Getrennte `skills-root/` vs. `runtime/` (§4.1).
- **HIGH — setting-sources Scope:** `local` raus, nur `project` (verifiziert: add-dir lädt
  weiterhin) → kleinere Angriffsfläche (§4.1).
- **HIGH — Launcher-Injektion/Windows:** definierter Argv-Encoder; `.ps1` via
  `-NoProfile -ExecutionPolicy Bypass`; `codex.cmd` explizit (nicht `.ps1`-Shim) (§4.4).
- **HIGH — MCP-Quelle/-Scope:** `claude mcp get` kein JSON → workdir-bezogen aus
  `~/.claude.json` (user + `projects[workdir]`) und `<workdir>/.mcp.json`; Plugin/managed hart
  deaktiviert; ID `{scope,name,sourcePath}` (§4.1).
- **HIGH — Command-Injection allgemein:** kein Shell-String aus User-Input; Launcher + argv
  (§4.4).
- **MED — ID-Kollisionen:** `SkillRef`/`McpRef` mit Scope/Pfad statt bloßer Namen (§5).
- **MED — Skill-Kopie-Härtung:** keine Symlinks/Junctions folgen, Caps (§4.1).
- **MED — Cleanup:** Lock/Heartbeat + TTL, nur beim App-Start, nie aktive Dirs (§6).
- **Codex-Flags:** gegen v0.140.0 verifiziert; `exec` = One-Shot → Keep-open (§4.3/§4.4).

**Noch offen / bei Umsetzung prüfen:**
- **Skill-Namens-Kollision zur Laufzeit (LOW):** gewählter Skill vs. gleichnamiger workdir-
  project-Skill (beides `verify`) — Claude-interne Präzedenz beim Laden prüfen, ggf. warnen.
- **macOS/Linux-Terminal-Spawn:** Fenster-/Quoting-Verhalten je Terminal testen (primär Windows
  entwickelt).
- **Workspace-Trust-Dialog (interaktiv):** für `--add-dir`-Pfade kann ein Trust-Dialog kommen;
  mit `-p` übersprungen. Erstes interaktives Verhalten prüfen.
- **Codex-MCP-Auth-Rauschen:** Subprozesse loggen `AuthRequired` für nicht-auth MCP-Server —
  kosmetisch.

## 10. Test / Verifikation

- Rust-Unit-Tests: argv-Assemblierung + Encoder (claude & codex) für repräsentative Profile.
- **Encoder-Sicherheit:** Prompt mit `"`, `;`, `$()`, Backticks, Newlines; Pfade mit
  Leerzeichen/`;`/Unicode → kein Brechen, keine Injektion (PS- und POSIX-Launcher).
- Manuell: Launch eines Research-Profils → im gestarteten Agent prüfen, dass `skills`
  (stream-json `init`, Methode §4.1) = **gewählte Skills + workdir-Projekt-Skills** (nicht
  „exakt nur gewählte" — §4.1/§4.2).
- **Windows-Spezifika:** `.ps1`-Launcher unter `RemoteSigned` startet (Bypass), `codex.cmd`
  wird aufgelöst (nicht `.ps1`).
- **Secrets:** der gestartete Agent kann `runtime/mcp.json` **nicht** über Datei-Tools lesen.
- **MCP-Scope-Kollision:** gleichnamige Server in user vs. project korrekt unterschieden.
- Round-trip: Profil speichern → App neu starten → Profil wieder da (Refs auflösbar).
- Codex-Launch: workdir + model korrekt, Skill-/MCP-Felder ignoriert.

## 11. Out of Scope (spätere Milestones)

- Eingebettetes Terminal-Panel (xterm.js + pty).
- Headless-Hintergrund-Runs mit Output-Capture im Skillet-UI.
- Durchreichen ausgewählter user-Settings (Permissions/Hooks) in die Isolation.
- Echte Skill-/MCP-Loadouts für Codex (sobald dessen Plugin-System das hergibt).
- Globaler SSOT-Modus (Mutation von `~/.claude`) als Alternative zur Isolation.
