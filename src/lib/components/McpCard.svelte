<script lang="ts">
  import type { McpServer } from "$lib/types";

  let { server, index }: { server: McpServer; index: number } = $props();

  const stashed = $derived(server.status === "stashed");

  const dotCls = $derived(
    server.status === "connected"
      ? "conn"
      : server.status === "needs-auth" || server.status === "pending"
        ? "auth"
        : server.status === "error"
          ? "err"
          : "grey",
  );
  const statusLabel = $derived(
    server.status === "connected"
      ? "verbunden"
      : server.status === "needs-auth"
        ? "Auth erforderlich"
        : server.status === "pending"
          ? "ausstehend"
          : server.status === "error"
            ? "Fehler"
            : server.status === "stashed"
              ? "stashed"
              : "unbekannt",
  );
  const scopeCls = $derived(
    server.scope === "plugin" || server.scope === "managed"
      ? "scope-plugin"
      : server.scope === "project"
        ? "scope-project"
        : "scope-personal",
  );
</script>

<article class="card stagger" style="animation-delay:{index * 28}ms">
  <div class="card-top">
    <div class="card-name-row">
      {#if server.managed}
        <svg class="lock-ico" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke-width="1.9">
          <rect x="5" y="11" width="14" height="9" rx="1.5" /><path d="M8 11V8a4 4 0 0 1 8 0v3" />
        </svg>
      {/if}
      <span class="card-name">{server.name}</span>
    </div>
    <span class="scope-badge {scopeCls}">{server.scope}</span>
  </div>

  <div class="endpoint">{server.endpoint}</div>

  <div class="mcp-meta">
    <span class="tag {server.transport}">{server.transport === "http" ? "HTTP" : "stdio"}</span>
    <span class="status-line"><span class="dot {dotCls}"></span>{statusLabel}</span>
    {#if server.tools}
      <span class="tag">{server.tools} Tools</span>
    {/if}
  </div>

  <div class="hair"></div>

  <div class="mcp-actions">
    {#if server.managed}
      <span class="lock-note">
        <svg class="lock-ico" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9">
          <rect x="5" y="11" width="14" height="9" rx="1.5" /><path d="M8 11V8a4 4 0 0 1 8 0v3" />
        </svg>
        Managed · read-only
      </span>
      <button class="btn btn-sm inert">Entfernen</button>
    {:else if stashed}
      <span class="lock-note">stashed</span>
      <button class="btn btn-sm btn-restore" disabled title="geplant für M2">Wiederherstellen</button>
    {:else}
      <span class="status-line"><span class="dot {dotCls}"></span>{statusLabel}</span>
      <button class="btn btn-sm btn-danger" disabled title="geplant für M2">Entfernen</button>
    {/if}
  </div>
</article>
