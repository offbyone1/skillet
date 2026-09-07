<script lang="ts">
  import type { Skill } from "$lib/types";

  let {
    skill,
    index,
    onfav,
    onopen,
    favorite = false,
    density = "cards",
  }: {
    skill: Skill;
    index: number;
    onfav?: (s: Skill) => void;
    onopen?: (s: Skill, e: Event) => void;
    favorite?: boolean;
    density?: "cards" | "list";
  } = $props();

  const fromPlugin = $derived(skill.scope === "plugin");
  const qTxt = $derived(
    skill.desc_quality === "good"
      ? "Good description"
      : skill.desc_quality === "warn"
        ? "Weak description"
        : "Poor description",
  );
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="card stagger {density} {onopen ? 'clickable' : ''}"
  style="animation-delay:{index * 28}ms"
  title={density === "list" ? skill.description : null}
  role={onopen ? "button" : null}
  tabindex={onopen ? 0 : null}
  onclick={(e) => onopen?.(skill, e)}
  onkeydown={(e) => onopen && (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onopen(skill, e))}
>
  <div class="card-top">
    <div class="card-name-row">
      <span class="qdot {skill.desc_quality}" title={qTxt}></span>
      <span class="card-name">{skill.name}</span>
    </div>
    {#if fromPlugin}
      <span class="scope-badge scope-plugin">plugin</span>
    {:else}
      <button
        type="button"
        class="fav {favorite ? 'on' : ''}"
        aria-label={favorite ? "Remove favorite" : "Mark as favorite"}
        aria-pressed={favorite}
        onclick={(e) => {
          e.stopPropagation();
          onfav?.(skill);
        }}
      >
        <svg width="17" height="17" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5" fill={favorite ? "currentColor" : "none"}>
          <path d="M12 3.3l2.7 5.5 6 .9-4.35 4.24 1.03 5.96L12 17.6l-5.38 2.83 1.03-5.96L3.3 9.7l6-.9z" />
        </svg>
      </button>
    {/if}
  </div>

  <p class="card-desc">{skill.description || "— no description —"}</p>

  <div class="hair"></div>

  <div class="card-bottom">
    <div class="stats">
      <div class="stat"><div class="v">{skill.size_human}</div><div class="k">Size</div></div>
      <div class="stat"><div class="v">{skill.file_count}</div><div class="k">Files</div></div>
      <div class="stat"><div class="v">{skill.modified}</div><div class="k">Modified</div></div>
    </div>
    {#if !fromPlugin}
      <span class="scope-badge scope-{skill.scope}">{skill.scope}</span>
    {/if}
  </div>
</div>
