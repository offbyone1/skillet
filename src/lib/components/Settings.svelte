<script lang="ts">
  import { settings, ui } from "$lib/settings.svelte";

  function close() {
    ui.open = false;
  }
</script>

{#snippet sw(on: boolean, toggle: () => void, label: string)}
  <button
    type="button"
    class="toggle {on ? 'on' : ''}"
    role="switch"
    aria-checked={on}
    aria-label={label}
    onclick={toggle}
  >
    <span class="knob"></span>
  </button>
{/snippet}

{#if ui.open}
  <div class="pop-backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
    <div class="pop pop-settings" role="dialog" aria-modal="true" tabindex="-1" style="--ox:{ui.origin.x}px; --oy:{ui.origin.y}px">
      <div class="pop-head">
        <div class="pop-title">
          <svg class="set-gear" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7">
            <circle cx="12" cy="12" r="3.2" />
            <path d="M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-1.8-.3 1.6 1.6 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.6 1.6 0 0 0-1-1.5 1.6 1.6 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0 .3-1.8 1.6 1.6 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.6 1.6 0 0 0 1.5-1 1.6 1.6 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.6 1.6 0 0 0 1.8.3H9a1.6 1.6 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.6 1.6 0 0 0 1 1.5 1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0-.3 1.8V9a1.6 1.6 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1z" />
          </svg>
          <h3>Settings</h3>
        </div>
        <button class="pop-close" aria-label="Close" onclick={close}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18" /><line x1="18" y1="6" x2="6" y2="18" /></svg>
        </button>
      </div>

      <div class="set-body">
        <div class="set-row">
          <div class="set-label">
            <div class="set-name">Appearance</div>
            <div class="set-hint">Dark or light theme</div>
          </div>
          <div class="seg">
            <button class="seg-btn {settings.theme === 'dark' ? 'active' : ''}" onclick={() => (settings.theme = "dark")}>Dark</button>
            <button class="seg-btn {settings.theme === 'light' ? 'active' : ''}" onclick={() => (settings.theme = "light")}>Light</button>
          </div>
        </div>

        <div class="set-row">
          <div class="set-label">
            <div class="set-name">Reduce motion</div>
            <div class="set-hint">Turn off animations and transitions</div>
          </div>
          {@render sw(settings.reduceMotion, () => (settings.reduceMotion = !settings.reduceMotion), "Reduce motion")}
        </div>

        <div class="set-row">
          <div class="set-label">
            <div class="set-name">Background pattern</div>
            <div class="set-hint">Skillet ASCII behind the content</div>
          </div>
          {@render sw(settings.showBackdrop, () => (settings.showBackdrop = !settings.showBackdrop), "Background pattern")}
        </div>
      </div>
    </div>
  </div>
{/if}
