<script lang="ts">
  import { onMount } from "svelte";
  import {
    scanSkills, listMcp, healthCheck, claudeVersion,
    listProfiles, saveProfile, deleteProfile, launchAgent, preflightWorkdir, pickFolder,
  } from "$lib/api";
  import type { Skill, McpServer, Finding, Loadout, SkillRef, McpRef, AgentKind } from "$lib/types";
  import SkillCard from "$lib/components/SkillCard.svelte";
  import McpCard from "$lib/components/McpCard.svelte";

  let view = $state<"skills" | "mcp" | "starter" | "health">("skills");
  let density = $state<"cards" | "list">("cards");
  let skills = $state<Skill[]>([]);
  let mcp = $state<McpServer[]>([]);
  let findings = $state<Finding[]>([]);
  let version = $state("…");
  let search = $state("");
  let loading = $state(true);
  let mcpLoading = $state(true);
  let errored = $state(false);

  const f = $derived(search.trim().toLowerCase());
  const match = (s: Skill) =>
    !f || s.name.toLowerCase().includes(f) || s.description.toLowerCase().includes(f);
  const onSkills = $derived(skills.filter((s) => s.enabled && match(s)));
  const offSkills = $derived(skills.filter((s) => !s.enabled && match(s)));

  // Plugin skills (caveman-*, superpowers:*, …) are sub-skills of one parent
  // plugin — fold them into collapsible groups so they don't drown the user's
  // own personal/project skills. Loose = no parent plugin.
  type PluginGroup = { plugin: string; skills: Skill[] };
  const loose = (list: Skill[]) => list.filter((s) => !s.plugin);
  function groupByPlugin(list: Skill[]): PluginGroup[] {
    const m = new Map<string, Skill[]>();
    for (const s of list) {
      if (!s.plugin) continue;
      const arr = m.get(s.plugin);
      if (arr) arr.push(s);
      else m.set(s.plugin, [s]);
    }
    return [...m.entries()]
      .map(([plugin, skills]) => ({ plugin, skills }))
      .sort((a, b) => a.plugin.localeCompare(b.plugin));
  }
  const onGroups = $derived(groupByPlugin(onSkills));
  const offLoose = $derived(loose(offSkills));
  const offGroups = $derived(groupByPlugin(offSkills));

  // Favourites: a starred subset of (non-plugin) skills, pinned into their own
  // section above "Aktiv". Keyed by path. In-memory for M0.
  let favorites = $state<Set<string>>(new Set());
  function toggleFav(s: Skill) {
    const next = new Set(favorites);
    next.has(s.path) ? next.delete(s.path) : next.add(s.path);
    favorites = next;
  }
  const favLoose = $derived(loose(onSkills).filter((s) => favorites.has(s.path)));
  const aktivLoose = $derived(loose(onSkills).filter((s) => !favorites.has(s.path)));

  // A plugin renders as a card stack (deck); clicking it pops out the individual
  // sub-skills. `openPlugin` holds the plugin whose pop-out is currently open.
  let openPlugin = $state<string | null>(null);
  const openGroup = $derived(
    [...onGroups, ...offGroups].find((g) => g.plugin === openPlugin) ?? null,
  );
  // Offset (deck-center → viewport-center) so the pop-out can animate as if
  // growing out of the card the user clicked, then settle in the screen centre.
  let popOrigin = $state({ x: 0, y: 0 });
  function originFrom(e: Event) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return {
      x: Math.round(r.left + r.width / 2 - window.innerWidth / 2),
      y: Math.round(r.top + r.height / 2 - window.innerHeight / 2),
    };
  }
  function openDeck(plugin: string, e: Event) {
    popOrigin = originFrom(e);
    openPlugin = plugin;
  }

  // Individual skill detail pop-out — every card is clickable and grows out of
  // itself, same animation as the plugin decks.
  let openSkill = $state<Skill | null>(null);
  function openSkillCard(s: Skill, e: Event) {
    popOrigin = originFrom(e);
    openSkill = s;
  }
  const skillQualityLabel = (q: string) =>
    q === "good" ? "Gute Beschreibung" : q === "warn" ? "Schwache Beschreibung" : "Dürftige Beschreibung";
  function toggleSkill(s: Skill) {
    // M0 is read-only on disk; this is an optimistic preview of M2 enable/disable.
    s.enabled = !s.enabled;
    skills = [...skills];
  }

  // A plugin is enabled/disabled as a unit — toggling flips all its sub-skills
  // (optimistic preview of M2). `e` may be a click on the deck's toggle, where
  // we must stop the deck from also opening its pop-out.
  function togglePlugin(g: { plugin: string; skills: Skill[] }, e?: Event) {
    e?.stopPropagation();
    const next = !g.skills.every((s) => s.enabled);
    for (const s of g.skills) s.enabled = next;
    skills = [...skills];
  }
  const pluginOn = (g: { skills: Skill[] }) => g.skills.every((s) => s.enabled);

  // ============ AGENT-STARTER ============
  let profiles = $state<Loadout[]>([]);
  let profilesLoading = $state(true);
  let editing = $state<Loadout | null>(null); // edit-modal form model (a clone)
  let editIsNew = $state(false);
  let launching = $state<Loadout | null>(null); // launch-modal target
  let launchWorkdir = $state("");
  let launchTrust = $state<string[]>([]);
  let launchBusy = $state(false);
  let launchErr = $state("");
  let toast = $state("");

  const builtins = $derived(profiles.filter((p) => p.builtin));
  const myLoadouts = $derived(profiles.filter((p) => !p.builtin));
  // MCP servers selectable for isolation: only re-specifiable scopes (§4.1).
  const isolatableMcp = $derived(
    mcp.filter((m) => !m.managed && ["user", "local", "project"].includes(m.scope)),
  );

  async function loadProfiles() {
    try {
      profiles = await listProfiles();
    } catch (e) {
      console.error(e);
    } finally {
      profilesLoading = false;
    }
  }

  function blankLoadout(): Loadout {
    return {
      id: crypto.randomUUID(),
      name: "",
      note: "",
      agent: "claude",
      workdir: "",
      skills: [],
      mcps: [],
      model: null,
      effort: null,
      prompt: null,
      skipPerms: false,
      builtin: false,
    };
  }

  function openEdit(p: Loadout | null) {
    editIsNew = p === null || p.builtin;
    // Clone so edits don't mutate the list until saved; a builtin becomes a new copy.
    const base = p ? structuredClone($state.snapshot(p)) : blankLoadout();
    if (!p || p.builtin) {
      base.id = crypto.randomUUID();
      base.builtin = false;
      if (p?.builtin) base.name = `${p.name} (Kopie)`;
    }
    editing = base;
  }

  const SKILL_KEY = (r: { name: string; path: string }) => (r.path ? `p:${r.path}` : `n:${r.name}`);
  const skillSelected = (s: Skill) =>
    !!editing && editing.skills.some((r) => SKILL_KEY(r) === SKILL_KEY(s) || r.name === s.name);
  function toggleSkillSel(s: Skill) {
    if (!editing) return;
    const has = editing.skills.some((r) => r.name === s.name);
    editing.skills = has
      ? editing.skills.filter((r) => r.name !== s.name)
      : [...editing.skills, { name: s.name, scope: s.scope, plugin: s.plugin, path: s.path }];
  }
  const mcpSelected = (m: McpServer) =>
    !!editing && editing.mcps.some((r) => r.name === m.name);
  function toggleMcpSel(m: McpServer) {
    if (!editing) return;
    const has = editing.mcps.some((r) => r.name === m.name);
    editing.mcps = has
      ? editing.mcps.filter((r) => r.name !== m.name)
      : [...editing.mcps, { name: m.name, scope: m.scope, sourcePath: "" }];
  }

  async function chooseEditWorkdir() {
    if (!editing) return;
    const dir = await pickFolder(editing.workdir);
    if (dir) editing.workdir = dir;
  }

  async function saveEdit() {
    if (!editing) return;
    if (!editing.name.trim()) {
      toast = "Name fehlt.";
      return;
    }
    try {
      await saveProfile($state.snapshot(editing) as Loadout);
      editing = null;
      await loadProfiles();
      toast = "Gespeichert.";
    } catch (e) {
      toast = `Speichern fehlgeschlagen: ${e}`;
    }
  }

  async function removeProfile(p: Loadout) {
    try {
      await deleteProfile(p.id);
      await loadProfiles();
      toast = `„${p.name}" gelöscht.`;
    } catch (e) {
      toast = `Löschen fehlgeschlagen: ${e}`;
    }
  }

  async function openLaunch(p: Loadout) {
    launching = p;
    launchWorkdir = p.workdir;
    launchErr = "";
    launchTrust = [];
    if (p.workdir) {
      try {
        launchTrust = await preflightWorkdir(p.workdir);
      } catch (e) {
        console.error(e);
      }
    }
  }
  async function chooseLaunchWorkdir() {
    const dir = await pickFolder(launchWorkdir);
    if (dir) {
      launchWorkdir = dir;
      try {
        launchTrust = await preflightWorkdir(dir);
      } catch (e) {
        console.error(e);
      }
    }
  }
  // Trust-gate (§4.2): launching with -p or skip-perms into a dir that carries
  // project hooks/CLAUDE.md/settings needs an explicit confirm.
  const launchRisky = $derived(
    !!launching && (!!launching.prompt?.trim() || launching.skipPerms) && launchTrust.length > 0,
  );
  let trustConfirmed = $state(false);

  async function confirmLaunch() {
    if (!launching) return;
    if (launchRisky && !trustConfirmed) {
      launchErr = "Bitte Vertrauen bestätigen.";
      return;
    }
    launchBusy = true;
    launchErr = "";
    try {
      const msg = await launchAgent(launching.id, launchWorkdir || null);
      toast = msg;
      launching = null;
      trustConfirmed = false;
    } catch (e) {
      launchErr = `${e}`;
    } finally {
      launchBusy = false;
    }
  }

  onMount(() => {
    // skills + version: fast local FS scan — render Skills view immediately.
    (async () => {
      try {
        const [s, v] = await Promise.all([scanSkills(), claudeVersion()]);
        skills = s;
        version = v;
      } catch (e) {
        console.error(e);
        errored = true;
      } finally {
        loading = false;
      }
    })();
    // mcp + health: `claude mcp list` runs network health-checks → may be slow,
    // so it loads separately and never blocks the Skills view.
    (async () => {
      try {
        const [m, h] = await Promise.all([listMcp(), healthCheck()]);
        mcp = m;
        findings = h;
      } catch (e) {
        console.error(e);
      } finally {
        mcpLoading = false;
      }
    })();
    // profiles: fast local file read in Skillet's own dir.
    loadProfiles();
  });
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      openPlugin = null;
      openSkill = null;
      editing = null;
      launching = null;
    }
  }}
/>

<div class="app">
  <!-- ====================== SIDEBAR ====================== -->
  <aside class="sidebar">
    <div class="wordmark">
      <svg width="34" height="34" viewBox="0 0 100 100" aria-hidden="true">
        <rect x="8" y="8" width="84" height="84" rx="20" fill="none" stroke="#c9a24b" stroke-width="5" />
        <circle cx="42" cy="50" r="14" fill="none" stroke="#e3c47e" stroke-width="5" />
        <line x1="56" y1="50" x2="80" y2="50" stroke="#c9a24b" stroke-width="5" stroke-linecap="round" />
      </svg>
      <div class="wm-text">
        <h1>Skillet</h1>
        <div class="wm-sub">Skills &amp; MCP</div>
      </div>
    </div>

    <nav class="nav">
      <div class="nav-label">Verwalten</div>
      <button class="nav-item {view === 'skills' ? 'active' : ''}" onclick={() => (view = "skills")}>
        <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M4 7h16M4 12h16M4 17h10" /></svg>
        Skills <span class="badge-count">{skills.length}</span>
      </button>
      <button class="nav-item {view === 'mcp' ? 'active' : ''}" onclick={() => (view = "mcp")}>
        <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="4" width="18" height="6" rx="1.5" /><rect x="3" y="14" width="18" height="6" rx="1.5" /><circle cx="7" cy="7" r="0.6" fill="currentColor" /><circle cx="7" cy="17" r="0.6" fill="currentColor" /></svg>
        MCPs <span class="badge-count">{mcp.length}</span>
      </button>
      <button class="nav-item {view === 'starter' ? 'active' : ''}" onclick={() => (view = "starter")}>
        <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 3l2.5 5.5L20 9l-4 4 1 6-5-3-5 3 1-6-4-4 5.5-.5z" /></svg>
        Agent-Starter
      </button>
    </nav>

    <div class="nav-spacer"></div>

    <div class="nav-secondary">
      <button class="nav-item secondary {view === 'health' ? 'active' : ''}" onclick={() => (view = "health")}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 12h4l2 6 4-14 2 8h6" /></svg>
        Health <span class="badge-count">{findings.length}</span>
      </button>
    </div>
    <div class="sidebar-foot">
      <div><b>{skills.length}</b> Skills &middot; <b>{mcp.length}</b> Server</div>
      <div>claude {version} &middot; M0 read-only</div>
    </div>
  </aside>

  <!-- ====================== MAIN CONTENT ====================== -->
  <main class="content">
    {#if view === "skills"}
      <section class="view">
        <div class="page-head">
          <h2>Skills</h2>
          <p>Alle erkannten Claude-Code-Skills. Aktivieren oder deaktivieren pro Eintrag. Plugin-Skills sind gesperrt.</p>
        </div>
        <div class="control-row">
          <div class="search-wrap">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke-width="1.7"><circle cx="11" cy="11" r="7" /><line x1="21" y1="21" x2="16.5" y2="16.5" /></svg>
            <input class="search" type="text" placeholder="Skills durchsuchen — Name oder Beschreibung" bind:value={search} />
          </div>
          <div class="legend" title="Farbpunkt links am Skill = Qualität seiner Beschreibung">
            <span class="legend-label">Beschreibung</span>
            <span><span class="dot good"></span>gut</span>
            <span><span class="dot warn"></span>schwach</span>
            <span><span class="dot bad"></span>dürftig</span>
          </div>
          <div class="seg" role="group" aria-label="Dichte">
            <button class="seg-btn {density === 'cards' ? 'active' : ''}" onclick={() => (density = "cards")}>Karten</button>
            <button class="seg-btn {density === 'list' ? 'active' : ''}" onclick={() => (density = "list")}>Liste</button>
          </div>
        </div>

        {#snippet deck(g: { plugin: string; skills: Skill[] }, i: number)}
          <div
            class="deck stagger {density} {pluginOn(g) ? '' : 'archived'}"
            style="animation-delay:{i * 28}ms"
            role="button"
            tabindex="0"
            onclick={(e) => openDeck(g.plugin, e)}
            onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), openDeck(g.plugin, e))}
            aria-label="{g.plugin} — {g.skills.length} Skills öffnen"
          >
            <span class="card-top">
              <span class="card-name-row">
                <svg class="deck-hex" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 2l8.66 5v10L12 22l-8.66-5V7z" /></svg>
                <span class="card-name">{g.plugin}</span>
              </span>
              <span class="scope-badge scope-plugin">plugin</span>
            </span>
            <span class="deck-desc">{g.skills.length} Skills · {g.skills.slice(0, 3).map((s) => s.name).join(", ")}{g.skills.length > 3 ? " …" : ""}</span>
            <span class="hair"></span>
            <span class="card-bottom">
              <span class="deck-open">Öffnen ›</span>
              <span class="toggle-wrap">
                <button
                  type="button"
                  class="toggle {pluginOn(g) ? 'on' : ''}"
                  role="switch"
                  aria-checked={pluginOn(g)}
                  aria-label="Plugin {g.plugin} {pluginOn(g) ? 'deaktivieren' : 'aktivieren'}"
                  onclick={(e) => togglePlugin(g, e)}
                >
                  <span class="knob"></span>
                </button>
                <span class="toggle-lbl">{pluginOn(g) ? "Aktiv" : "Aus"}</span>
              </span>
            </span>
          </div>
        {/snippet}

        {#if favLoose.length}
          <div class="group-head"><h3>Favoriten</h3><span class="cnt">{favLoose.length}</span><div class="rule"></div></div>
          <div class="grid {density}">
            {#each favLoose as s, i (s.path)}
              <SkillCard skill={s} index={i} ontoggle={toggleSkill} onfav={toggleFav} onopen={openSkillCard} favorite {density} />
            {/each}
          </div>
          <div class="sec-divider"></div>
        {/if}

        <div class="group-head"><h3>Aktiv</h3><span class="cnt">{onSkills.length} {onSkills.length === 1 ? "Skill" : "Skills"}</span><div class="rule"></div></div>
        {#if loading}
          <div class="grid"><div class="empty">Scanne Skills …</div></div>
        {:else if errored}
          <div class="grid"><div class="empty">Scan fehlgeschlagen — läuft die App über `tauri dev`?</div></div>
        {:else if onSkills.length}
          {#if onGroups.length}
            <div class="deck-grid {density}">
              {#each onGroups as g, i (g.plugin)}
                {@render deck(g, i)}
              {/each}
            </div>
            {#if aktivLoose.length}<div class="sec-divider"></div>{/if}
          {/if}
          {#if aktivLoose.length}
            <div class="grid {density}">
              {#each aktivLoose as s, i (s.path)}
                <SkillCard skill={s} index={i} ontoggle={toggleSkill} onfav={toggleFav} onopen={openSkillCard} favorite={favorites.has(s.path)} {density} />
              {/each}
            </div>
          {/if}
        {:else}
          <div class="grid"><div class="empty">Keine aktiven Skills gefunden.</div></div>
        {/if}

        <div class="group-head"><h3>Deaktiviert</h3><span class="cnt">{offSkills.length} {offSkills.length === 1 ? "Skill" : "Skills"}</span><div class="rule"></div></div>
        {#if offSkills.length}
          {#if offGroups.length}
            <div class="deck-grid {density}">
              {#each offGroups as g, i (g.plugin)}
                {@render deck(g, i)}
              {/each}
            </div>
            {#if offLoose.length}<div class="sec-divider"></div>{/if}
          {/if}
          {#if offLoose.length}
            <div class="grid {density}">
              {#each offLoose as s, i (s.path)}
                <SkillCard skill={s} index={i} ontoggle={toggleSkill} onfav={toggleFav} onopen={openSkillCard} favorite={favorites.has(s.path)} {density} />
              {/each}
            </div>
          {/if}
        {:else}
          <div class="grid"><div class="empty">Nichts deaktiviert.</div></div>
        {/if}
      </section>
    {/if}

    {#if view === "mcp"}
      <section class="view">
        <div class="page-head">
          <h2>MCP Server</h2>
          <p>Registrierte Model-Context-Protocol-Server. <b>plugin:*</b> und <b>claude.ai</b>-Server sind verwaltet und nur lesbar.</p>
        </div>
        <div class="mcp-grid">
          {#if mcpLoading}
            <div class="empty">Lese `claude mcp list` …</div>
          {:else if mcp.length}
            {#each mcp as m, i (m.name)}
              <McpCard server={m} index={i} />
            {/each}
          {:else}
            <div class="empty">Keine MCP-Server gefunden.</div>
          {/if}
        </div>
      </section>
    {/if}

    {#if view === "starter"}
      <section class="view">
        <div class="page-head">
          <h2>Agent-Starter</h2>
          <p>Loadout-Profile starten <b>claude</b> oder <b>codex</b> im gewählten Verzeichnis mit genau diesen Skills + MCPs. Isoliert pro Start — deine globale Config bleibt unberührt.</p>
        </div>

        {#snippet profileCard(p, i)}
          <div class="profile-card stagger" style="animation-delay:{i * 40}ms">
            <div class="pc-name">{p.name}</div>
            <div class="pc-note">{p.note || "—"}</div>
            <div class="pc-counts">
              <div class="pc-count"><span class="n">{p.skills.length}</span><span class="l">Skills</span></div>
              <div class="pc-count"><span class="n">{p.mcps.length}</span><span class="l">MCPs</span></div>
              <div class="pc-count"><span class="n">{p.agent}</span><span class="l">Agent</span></div>
            </div>
            <div class="pc-chips">
              {#if p.model}<span class="chip">{p.model}</span>{/if}
              {#if p.effort && p.agent === "claude"}<span class="chip">effort: {p.effort}</span>{/if}
              {#if p.prompt?.trim()}<span class="chip">headless</span>{/if}
              {#if p.skipPerms}<span class="chip">skip-perms</span>{/if}
              {#if p.builtin}<span class="chip mcp">Template</span>{/if}
            </div>
            <div class="pc-actions">
              <button class="btn btn-accent" onclick={() => openLaunch(p)}>Starten</button>
              {#if p.builtin}
                <button class="btn" onclick={() => openEdit(p)}>Duplizieren</button>
              {:else}
                <button class="btn" onclick={() => openEdit(p)}>Bearbeiten</button>
                <button class="btn btn-sm" aria-label="Löschen" onclick={() => removeProfile(p)}>
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14" /></svg>
                </button>
              {/if}
            </div>
          </div>
        {/snippet}

        {#if profilesLoading}
          <div class="empty">Lade Profile …</div>
        {:else}
          <div class="group-head"><h3>Templates</h3><span class="cnt">{builtins.length}</span><div class="rule"></div></div>
          <div class="profile-grid">
            {#each builtins as p, i (p.id)}
              {@render profileCard(p, i)}
            {/each}
          </div>

          <div class="group-head"><h3>Meine Loadouts</h3><span class="cnt">{myLoadouts.length}</span><div class="rule"></div></div>
          <div class="profile-grid">
            {#each myLoadouts as p, i (p.id)}
              {@render profileCard(p, i)}
            {/each}
            <button class="profile-card new" onclick={() => openEdit(null)}>
              <span class="plus">+</span>
              <span class="nt">Neues Loadout</span>
            </button>
          </div>
        {/if}
      </section>
    {/if}

    {#if view === "health"}
      <section class="view">
        <div class="page-head">
          <h2>Health</h2>
          <p>Erkannte Befunde aus dem letzten Scan.</p>
        </div>
        <div class="group-head"><h3>Befunde</h3><span class="cnt">{findings.length} offen</span><div class="rule"></div></div>
        <div class="findings">
          {#if mcpLoading}
            <div class="empty">Analysiere …</div>
          {:else if findings.length}
            {#each findings as fd, i (fd.title)}
              <div class="finding {fd.severity} stagger" style="animation-delay:{i * 40}ms">
                <div class="finding-body">
                  <div class="finding-title">{fd.title}</div>
                  <div class="finding-detail">{fd.detail}</div>
                </div>
                <span class="sev-tag {fd.severity}">{fd.severity}</span>
              </div>
            {/each}
          {:else}
            <div class="empty">Keine Befunde — alles sauber.</div>
          {/if}
        </div>
      </section>
    {/if}
  </main>
</div>

<!-- pop-out lives at the top level so `position: fixed` is viewport-relative
     (a transformed ancestor like .view would otherwise become its containing
     block and break centring). It grows out of the clicked deck via --ox/--oy. -->
{#if openGroup}
  <div
    class="pop-backdrop"
    role="presentation"
    onclick={(e) => e.target === e.currentTarget && (openPlugin = null)}
  >
    <div class="pop" role="dialog" aria-modal="true" tabindex="-1" style="--ox:{popOrigin.x}px; --oy:{popOrigin.y}px">
      <div class="pop-head">
        <div class="pop-title">
          <svg class="deck-hex" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 2l8.66 5v10L12 22l-8.66-5V7z" /></svg>
          <h3>{openGroup.plugin}</h3>
          <span class="pop-cnt">{openGroup.skills.length} Skills</span>
        </div>
        <div class="pop-actions">
          <span class="toggle-wrap">
            <button
              type="button"
              class="toggle {pluginOn(openGroup) ? 'on' : ''}"
              role="switch"
              aria-checked={pluginOn(openGroup)}
              aria-label="Plugin {openGroup.plugin} {pluginOn(openGroup) ? 'deaktivieren' : 'aktivieren'}"
              onclick={() => togglePlugin(openGroup)}
            >
              <span class="knob"></span>
            </button>
            <span class="toggle-lbl">{pluginOn(openGroup) ? "Aktiv" : "Aus"}</span>
          </span>
          <button class="pop-close" aria-label="Schließen" onclick={() => (openPlugin = null)}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18" /><line x1="18" y1="6" x2="6" y2="18" /></svg>
          </button>
        </div>
      </div>
      <div class="pop-body grid">
        {#each openGroup.skills as s, i (s.path)}
          <SkillCard skill={s} index={i} ontoggle={toggleSkill} />
        {/each}
      </div>
    </div>
  </div>
{/if}

<!-- individual skill detail pop-out -->
{#if openSkill}
  <div class="pop-backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && (openSkill = null)}>
    <div class="pop pop-skill" role="dialog" aria-modal="true" tabindex="-1" style="--ox:{popOrigin.x}px; --oy:{popOrigin.y}px">
      <div class="pop-head">
        <div class="pop-title">
          <span class="qdot {openSkill.desc_quality}" title={skillQualityLabel(openSkill.desc_quality)}></span>
          <h3>{openSkill.name}</h3>
          <span class="scope-badge scope-{openSkill.scope}">{openSkill.plugin ?? openSkill.scope}</span>
        </div>
        <button class="pop-close" aria-label="Schließen" onclick={() => (openSkill = null)}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18" /><line x1="18" y1="6" x2="6" y2="18" /></svg>
        </button>
      </div>
      <div class="pop-skill-body">
        <p class="pop-desc">{openSkill.description || "— keine Beschreibung —"}</p>
        <div class="pop-stats">
          <div class="stat"><div class="v">{openSkill.size_human}</div><div class="k">Größe</div></div>
          <div class="stat"><div class="v">{openSkill.file_count}</div><div class="k">Dateien</div></div>
          <div class="stat"><div class="v">{openSkill.modified}</div><div class="k">Geändert</div></div>
          <div class="stat"><div class="v">{skillQualityLabel(openSkill.desc_quality).split(" ")[0]}</div><div class="k">Desc</div></div>
        </div>
        {#if openSkill.allowed_tools}
          <div class="pop-field">
            <div class="pop-field-k">Erlaubte Tools</div>
            <div class="pop-field-v">{openSkill.allowed_tools}</div>
          </div>
        {/if}
        <div class="pop-field">
          <div class="pop-field-k">Pfad</div>
          <div class="pop-field-v mono">{openSkill.path}</div>
        </div>
      </div>
    </div>
  </div>
{/if}
