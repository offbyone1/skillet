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
      ? "connected"
      : server.status === "needs-auth"
        ? "auth required"
        : server.status === "pending"
          ? "pending"
          : server.status === "error"
            ? "error"
            : server.status === "stashed"
              ? "stashed"
              : "unknown",
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
      <button class="btn btn-sm inert">Remove</button>
    {:else if stashed}
      <span class="lock-note">stashed</span>
      <button class="btn btn-sm btn-restore" disabled title="planned for M2">Restore</button>
    {:else}
      <span class="status-line"><span class="dot {dotCls}"></span>{statusLabel}</span>
      <button class="btn btn-sm btn-danger" disabled title="planned for M2">Remove</button>
    {/if}
  </div>
</article>
