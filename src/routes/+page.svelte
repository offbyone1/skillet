<script lang="ts">
  import { onMount } from "svelte";
  import {
    scanSkills, listMcp, healthCheck, claudeVersion,
    listProfiles, saveProfile, deleteProfile, launchAgent, preflightWorkdir, pickFolder,
  } from "$lib/api";
  import type { Skill, McpServer, Finding, Loadout, SkillRef, McpRef, AgentKind } from "$lib/types";
  import SkillCard from "$lib/components/SkillCard.svelte";
  import McpCard from "$lib/components/McpCard.svelte";
  import { ui as settingsUi } from "$lib/settings.svelte";

  // open the settings pop-out, growing it out of the sidebar button
  function openSettings(e: Event) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    settingsUi.origin = {
      x: Math.round(r.left + r.width / 2 - window.innerWidth / 2),
      y: Math.round(r.top + r.height / 2 - window.innerHeight / 2),
    };
    settingsUi.open = true;
  }

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
  const shown = $derived(skills.filter(match));

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
  const groups = $derived(groupByPlugin(shown));

  // Favourites: a starred subset of (non-plugin) skills, pinned into their own
  // section above the rest. Keyed by path, in-memory only.
  let favorites = $state<Set<string>>(new Set());
  function toggleFav(s: Skill) {
    const next = new Set(favorites);
    next.has(s.path) ? next.delete(s.path) : next.add(s.path);
    favorites = next;
  }
  const favLoose = $derived(loose(shown).filter((s) => favorites.has(s.path)));
  const restLoose = $derived(loose(shown).filter((s) => !favorites.has(s.path)));

  // A plugin renders as a card stack (deck); clicking it pops out the individual
  // sub-skills. `openPlugin` holds the plugin whose pop-out is currently open.
  let openPlugin = $state<string | null>(null);
  const openGroup = $derived(
    groups.find((g) => g.plugin === openPlugin) ?? null,
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
    q === "good" ? "Good description" : q === "warn" ? "Weak description" : "Poor description";

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
      if (p?.builtin) base.name = `${p.name} (copy)`;
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
      toast = "Name is missing.";
      return;
    }
    try {
      await saveProfile($state.snapshot(editing) as Loadout);
      editing = null;
      await loadProfiles();
      toast = "Saved.";
    } catch (e) {
      toast = `Save failed: ${e}`;
    }
  }

  async function removeProfile(p: Loadout) {
    try {
      await deleteProfile(p.id);
      await loadProfiles();
      toast = `"${p.name}" deleted.`;
    } catch (e) {
      toast = `Delete failed: ${e}`;
    }
  }

  async function refreshTrust(dir: string) {
    trustConfirmed = false; // a changed dir invalidates a prior confirmation
    launchTrust = [];
    if (!dir.trim()) return;
    try {
      launchTrust = await preflightWorkdir(dir);
    } catch (e) {
      console.error(e);
    }
  }
  async function openLaunch(p: Loadout) {
    launching = p;
    launchWorkdir = p.workdir;
    launchErr = "";
    await refreshTrust(p.workdir);
  }
  async function chooseLaunchWorkdir() {
    const dir = await pickFolder(launchWorkdir);
    if (dir) {
      launchWorkdir = dir;
      await refreshTrust(dir);
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
      launchErr = "Please confirm trust.";
      return;
    }
    launchBusy = true;
    launchErr = "";
    try {
      const msg = await launchAgent(launching.id, launchWorkdir || null, trustConfirmed);
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
      <div class="nav-label">Manage</div>
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
        Agent Starter
      </button>
    </nav>

    <div class="nav-spacer"></div>

    <div class="nav-secondary">
      <button class="nav-item secondary {view === 'health' ? 'active' : ''}" onclick={() => (view = "health")}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 12h4l2 6 4-14 2 8h6" /></svg>
        Health <span class="badge-count">{findings.length}</span>
      </button>
      <button class="nav-item secondary" onclick={openSettings}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3" /><path d="M19.4 13.5a7.8 7.8 0 0 0 0-3l1.8-1.4-2-3.4-2.1.9a7.6 7.6 0 0 0-2.6-1.5L13.9 2h-3.8l-.6 2.1a7.6 7.6 0 0 0-2.6 1.5l-2.1-.9-2 3.4 1.8 1.4a7.8 7.8 0 0 0 0 3L2.8 15l2 3.4 2.1-.9a7.6 7.6 0 0 0 2.6 1.5l.6 2.1h3.8l.6-2.1a7.6 7.6 0 0 0 2.6-1.5l2.1.9 2-3.4z" /></svg>
        Settings
      </button>
    </div>
    <div class="sidebar-foot">
      <div><b>{skills.length}</b> Skills &middot; <b>{mcp.length}</b> Server</div>
      <div>claude {version} &middot; read-only</div>
    </div>
  </aside>

  <!-- ====================== MAIN CONTENT ====================== -->
  <main class="content">
    {#if view === "skills"}
      <section class="view">
        <div class="page-head">
          <h2>Skills</h2>
          <p>Every Claude Code skill on this machine — personal, project and plugin. Skillet reads them; it changes nothing.</p>
        </div>
        <div class="control-row">
          <div class="search-wrap">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke-width="1.7"><circle cx="11" cy="11" r="7" /><line x1="21" y1="21" x2="16.5" y2="16.5" /></svg>
            <input class="search" type="text" placeholder="Search skills — name or description" bind:value={search} />
          </div>
          <div class="legend" title="Color dot left of a skill = quality of its description">
            <span class="legend-label">Description</span>
            <span><span class="dot good"></span>good</span>
            <span><span class="dot warn"></span>weak</span>
            <span><span class="dot bad"></span>poor</span>
          </div>
          <div class="seg" role="group" aria-label="Density">
            <button class="seg-btn {density === 'cards' ? 'active' : ''}" onclick={() => (density = "cards")}>Cards</button>
            <button class="seg-btn {density === 'list' ? 'active' : ''}" onclick={() => (density = "list")}>List</button>
          </div>
        </div>

        {#snippet deck(g: { plugin: string; skills: Skill[] }, i: number)}
          <div
            class="deck stagger {density}"
            style="animation-delay:{i * 28}ms"
            role="button"
            tabindex="0"
            onclick={(e) => openDeck(g.plugin, e)}
            onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), openDeck(g.plugin, e))}
            aria-label="{g.plugin} — open {g.skills.length} skills"
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
              <span class="deck-open">Open ›</span>
              <span class="toggle-lbl">{g.skills.length} {g.skills.length === 1 ? "skill" : "skills"}</span>
            </span>
          </div>
        {/snippet}

        {#if favLoose.length}
          <div class="group-head"><h3>Favorites</h3><span class="cnt">{favLoose.length}</span><div class="rule"></div></div>
          <div class="grid {density}">
            {#each favLoose as s, i (s.path)}
              <SkillCard skill={s} index={i} onfav={toggleFav} onopen={openSkillCard} favorite {density} />
            {/each}
          </div>
          <div class="sec-divider"></div>
        {/if}

        <div class="group-head"><h3>Installed</h3><span class="cnt">{shown.length} {shown.length === 1 ? "Skill" : "Skills"}</span><div class="rule"></div></div>
        {#if loading}
          <div class="grid"><div class="empty">Scanning skills …</div></div>
        {:else if errored}
          <div class="grid"><div class="empty">Scan failed — is the Claude Code CLI installed?</div></div>
        {:else if shown.length}
          {#if groups.length}
            <div class="deck-grid {density}">
              {#each groups as g, i (g.plugin)}
                {@render deck(g, i)}
              {/each}
            </div>
            {#if restLoose.length}<div class="sec-divider"></div>{/if}
          {/if}
          {#if restLoose.length}
            <div class="grid {density}">
              {#each restLoose as s, i (s.path)}
                <SkillCard skill={s} index={i} onfav={toggleFav} onopen={openSkillCard} favorite={favorites.has(s.path)} {density} />
              {/each}
            </div>
          {/if}
        {:else}
          <div class="grid"><div class="empty">No skills found.</div></div>
        {/if}
      </section>
    {/if}

    {#if view === "mcp"}
      <section class="view">
        <div class="page-head">
          <h2>MCP Server</h2>
          <p>Registered Model Context Protocol servers. <b>plugin:*</b> and <b>claude.ai</b> servers are managed and read-only.</p>
        </div>
        <div class="mcp-grid">
          {#if mcpLoading}
            <div class="empty">Reading `claude mcp list` …</div>
          {:else if mcp.length}
            {#each mcp as m, i (m.name)}
              <McpCard server={m} index={i} />
            {/each}
          {:else}
            <div class="empty">No MCP servers found.</div>
          {/if}
        </div>
      </section>
    {/if}

    {#if view === "starter"}
      <section class="view">
        <div class="page-head">
          <h2>Agent Starter</h2>
          <p>Loadout profiles launch <b>claude</b> or <b>codex</b> in the chosen directory with exactly these skills + MCPs. Isolated per launch — your global config stays untouched.</p>
        </div>

        {#snippet profileCard(p: Loadout, i: number)}
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
              <button class="btn btn-accent" onclick={() => openLaunch(p)}>Launch</button>
              {#if p.builtin}
                <button class="btn" onclick={() => openEdit(p)}>Duplicate</button>
              {:else}
                <button class="btn" onclick={() => openEdit(p)}>Edit</button>
                <button class="btn btn-sm" aria-label="Delete" onclick={() => removeProfile(p)}>
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14" /></svg>
                </button>
              {/if}
            </div>
          </div>
        {/snippet}

        {#if profilesLoading}
          <div class="empty">Loading profiles …</div>
        {:else}
          <div class="group-head"><h3>Templates</h3><span class="cnt">{builtins.length}</span><div class="rule"></div></div>
          <div class="profile-grid">
            {#each builtins as p, i (p.id)}
              {@render profileCard(p, i)}
            {/each}
          </div>

          <div class="group-head"><h3>My Loadouts</h3><span class="cnt">{myLoadouts.length}</span><div class="rule"></div></div>
          <div class="profile-grid">
            {#each myLoadouts as p, i (p.id)}
              {@render profileCard(p, i)}
            {/each}
            <button class="profile-card new" onclick={() => openEdit(null)}>
              <span class="plus">+</span>
              <span class="nt">New Loadout</span>
            </button>
          </div>
        {/if}
      </section>
    {/if}

    {#if view === "health"}
      <section class="view">
        <div class="page-head">
          <h2>Health</h2>
          <p>Detected findings from the last scan.</p>
        </div>
        <div class="group-head"><h3>Findings</h3><span class="cnt">{findings.length} open</span><div class="rule"></div></div>
        <div class="findings">
          {#if mcpLoading}
            <div class="empty">Analyzing …</div>
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
            <div class="empty">No findings — all clean.</div>
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
          <button class="pop-close" aria-label="Close" onclick={() => (openPlugin = null)}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18" /><line x1="18" y1="6" x2="6" y2="18" /></svg>
          </button>
        </div>
      </div>
      <div class="pop-body grid">
        {#each openGroup.skills as s, i (s.path)}
          <SkillCard skill={s} index={i} />
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
        <button class="pop-close" aria-label="Close" onclick={() => (openSkill = null)}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18" /><line x1="18" y1="6" x2="6" y2="18" /></svg>
        </button>
      </div>
      <div class="pop-skill-body">
        <p class="pop-desc">{openSkill.description || "— no description —"}</p>
        <div class="pop-stats">
          <div class="stat"><div class="v">{openSkill.size_human}</div><div class="k">Size</div></div>
          <div class="stat"><div class="v">{openSkill.file_count}</div><div class="k">Files</div></div>
          <div class="stat"><div class="v">{openSkill.modified}</div><div class="k">Modified</div></div>
          <div class="stat"><div class="v">{skillQualityLabel(openSkill.desc_quality).split(" ")[0]}</div><div class="k">Desc</div></div>
        </div>
        {#if openSkill.allowed_tools}
          <div class="pop-field">
            <div class="pop-field-k">Allowed tools</div>
            <div class="pop-field-v">{openSkill.allowed_tools}</div>
          </div>
        {/if}
        <div class="pop-field">
          <div class="pop-field-k">Path</div>
          <div class="pop-field-v mono">{openSkill.path}</div>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- ====================== EDIT LOADOUT MODAL ====================== -->
{#if editing}
  <div class="backdrop show" role="presentation" onclick={(e) => e.target === e.currentTarget && (editing = null)}>
    <div class="modal wide" role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-head">
        <div>
          <h3>{editIsNew ? "New Loadout" : "Edit Loadout"}</h3>
          <div class="sub">Launches <b>{editing.agent}</b> with exactly this set.</div>
        </div>
        <button class="modal-close" aria-label="Close" onclick={() => (editing = null)}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18" /><line x1="18" y1="6" x2="6" y2="18" /></svg>
        </button>
      </div>
      <div class="modal-body">
        <div class="form-row">
          <div class="ff grow"><label for="lo-name">Name</label><input id="lo-name" bind:value={editing.name} placeholder="e.g. Research" /></div>
          <div class="ff"><label for="lo-agent">Agent</label>
            <select id="lo-agent" bind:value={editing.agent}>
              <option value="claude">claude</option>
              <option value="codex">codex</option>
            </select>
          </div>
        </div>
        <div class="name-field"><label for="lo-note">Note</label><input id="lo-note" bind:value={editing.note} placeholder="short, optional" /></div>

        <div class="form-row">
          <div class="ff grow"><label for="lo-model">Model</label>
            <input id="lo-model" bind:value={editing.model} placeholder={editing.agent === "codex" ? "e.g. gpt-5.5" : "opus / sonnet / haiku / fable"} />
          </div>
          <div class="ff"><label for="lo-effort">Effort {editing.agent !== "claude" ? "(claude)" : ""}</label>
            <select id="lo-effort" bind:value={editing.effort} disabled={editing.agent !== "claude"}>
              <option value={null}>—</option>
              <option value="low">low</option>
              <option value="medium">medium</option>
              <option value="high">high</option>
              <option value="xhigh">xhigh</option>
              <option value="max">max</option>
            </select>
          </div>
        </div>

        <div class="name-field"><label for="lo-workdir">Working directory (default — overridable at launch)</label>
          <div class="wd-row">
            <input id="lo-workdir" bind:value={editing.workdir} placeholder="C:\\Users\\…\\project" />
            <button class="btn" onclick={chooseEditWorkdir}>Choose…</button>
          </div>
        </div>

        <div class="name-field"><label for="lo-prompt">Prompt (empty = interactive · set = headless <code>-p</code>)</label>
          <textarea id="lo-prompt" class="ta" bind:value={editing.prompt} rows="2" placeholder="optional start prompt"></textarea>
        </div>

        <div class="toggle-row">
          <span class="toggle-wrap">
            <button type="button" class="toggle {editing.skipPerms ? 'on' : ''}" role="switch" aria-checked={editing.skipPerms} aria-label="Launch with --dangerously-skip-permissions" onclick={() => editing && (editing.skipPerms = !editing.skipPerms)}>
              <span class="knob"></span>
            </button>
            <span class="toggle-lbl"><code>--dangerously-skip-permissions</code></span>
          </span>
        </div>

        {#if editing.agent === "claude"}
          <div class="picker-cols">
            <div class="picker-col">
              <h4>Skills <span class="pc-n">{editing.skills.length}</span></h4>
              <div class="picker-list">
                {#if loading}
                  <div class="pick-empty">Scanning …</div>
                {:else}
                  {#each skills as s (s.path)}
                    <div class="pick-row {skillSelected(s) ? 'checked' : ''}" role="button" tabindex="0"
                      onclick={() => toggleSkillSel(s)}
                      onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), toggleSkillSel(s))}>
                      <span class="pick-check"><svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="#0b0d10" stroke-width="3"><path d="M5 12l5 5 9-11" /></svg></span>
                      <span class="pick-main"><span class="pick-name">{s.name}</span><span class="pick-sub">{s.description}</span></span>
                      <span class="pick-scope">{s.plugin ?? s.scope}</span>
                    </div>
                  {/each}
                {/if}
              </div>
            </div>
            <div class="picker-col">
              <h4>MCPs <span class="pc-n">{editing.mcps.length}</span></h4>
              <div class="picker-list">
                {#if mcpLoading}
                  <div class="pick-empty">Reading MCP servers …</div>
                {:else if isolatableMcp.length}
                  {#each isolatableMcp as m (m.name)}
                    <div class="pick-row {mcpSelected(m) ? 'checked' : ''}" role="button" tabindex="0"
                      onclick={() => toggleMcpSel(m)}
                      onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), toggleMcpSel(m))}>
                      <span class="pick-check"><svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="#0b0d10" stroke-width="3"><path d="M5 12l5 5 9-11" /></svg></span>
                      <span class="pick-main"><span class="pick-name">{m.name}</span><span class="pick-sub">{m.endpoint}</span></span>
                      <span class="pick-scope">{m.scope}</span>
                    </div>
                  {/each}
                {:else}
                  <div class="pick-empty">No isolatable servers.</div>
                {/if}
              </div>
              <p class="hint">Plugin/managed servers (<code>plugin:*</code>, <code>claude.ai</code>) are not isolatable and are hidden.</p>
            </div>
          </div>
        {:else}
          <div class="callout">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke-width="1.8"><circle cx="12" cy="12" r="9" /><path d="M12 8v5M12 16h.01" /></svg>
            <p>Skill and MCP isolation apply to <b>claude</b> only. Codex uses its own plugin/MCP system — here only directory, model, prompt and skip-perms matter.</p>
          </div>
        {/if}
      </div>
      <div class="modal-foot">
        <span class="spacer"></span>
        <button class="btn" onclick={() => (editing = null)}>Cancel</button>
        <button class="btn btn-accent" onclick={saveEdit}>Save</button>
      </div>
    </div>
  </div>
{/if}

<!-- ====================== LAUNCH MODAL ====================== -->
{#if launching}
  <div class="backdrop show" role="presentation" onclick={(e) => e.target === e.currentTarget && (launching = null)}>
    <div class="modal" role="dialog" aria-modal="true" tabindex="-1" style="max-width:540px">
      <div class="modal-head">
        <div>
          <h3>Launch: {launching.name}</h3>
          <div class="sub">{launching.agent}{launching.prompt?.trim() ? " · headless" : " · interactive"}</div>
        </div>
        <button class="modal-close" aria-label="Close" onclick={() => (launching = null)}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18" /><line x1="18" y1="6" x2="6" y2="18" /></svg>
        </button>
      </div>
      <div class="modal-body">
        <div class="name-field"><label for="launch-workdir">Working directory</label>
          <div class="wd-row">
            <input id="launch-workdir" bind:value={launchWorkdir} placeholder="Choose a directory…" onchange={() => refreshTrust(launchWorkdir)} />
            <button class="btn" onclick={chooseLaunchWorkdir}>Choose…</button>
          </div>
        </div>
        {#if launchTrust.length}
          <div class="callout">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke-width="1.8"><path d="M12 3l8 4v5c0 5-3.5 8-8 9-4.5-1-8-4-8-9V7z" /></svg>
            <p><b>Project config in this directory:</b> {launchTrust.join(", ")}. These are loaded and executed/honored by the agent.</p>
          </div>
        {/if}
        {#if launchRisky}
          <label class="trust-confirm">
            <input type="checkbox" bind:checked={trustConfirmed} />
            I trust this directory — launch with {launching.skipPerms ? "skip-permissions" : ""}{launching.skipPerms && launching.prompt?.trim() ? " + " : ""}{launching.prompt?.trim() ? "headless" : ""}.
          </label>
        {/if}
        {#if launchErr}<div class="launch-err">{launchErr}</div>{/if}
      </div>
      <div class="modal-foot">
        <span class="spacer"></span>
        <button class="btn" onclick={() => (launching = null)}>Cancel</button>
        <button class="btn btn-accent" disabled={launchBusy} onclick={confirmLaunch}>{launchBusy ? "Launching…" : "Launch"}</button>
      </div>
    </div>
  </div>
{/if}

{#if toast}
  <div class="toast" role="status">
    <span>{toast}</span>
    <button aria-label="Close" onclick={() => (toast = "")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18" /><line x1="18" y1="6" x2="6" y2="18" /></svg>
    </button>
  </div>
{/if}
