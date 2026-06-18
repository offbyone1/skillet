<script lang="ts">
  import "$lib/global.css";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Settings from "$lib/components/Settings.svelte";
  import { settings, applySettings, persist } from "$lib/settings.svelte";

  let { children } = $props();

  // custom window chrome (decorations are off in tauri.conf.json)
  const appWindow = getCurrentWindow();

  // keep <html> data-* attributes and localStorage in sync with settings
  $effect(() => {
    void [settings.theme, settings.reduceMotion, settings.showBackdrop];
    applySettings();
    persist();
  });
</script>

<!-- thin transparent drag strip across the top; only the controls at the right
     are clickable, the rest drags the window -->
<div class="titlebar" data-tauri-drag-region>
  <div class="win-controls">
    <button class="win-btn" aria-label="Minimize" onclick={() => appWindow.minimize()}>
      <svg width="11" height="11" viewBox="0 0 12 12"><line x1="2" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.2" /></svg>
    </button>
    <button class="win-btn" aria-label="Maximize" onclick={() => appWindow.toggleMaximize()}>
      <svg width="11" height="11" viewBox="0 0 12 12"><rect x="2.5" y="2.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
    </button>
    <button class="win-btn close" aria-label="Close" onclick={() => appWindow.close()}>
      <svg width="11" height="11" viewBox="0 0 12 12"><line x1="2.5" y1="2.5" x2="9.5" y2="9.5" stroke="currentColor" stroke-width="1.2" /><line x1="9.5" y1="2.5" x2="2.5" y2="9.5" stroke="currentColor" stroke-width="1.2" /></svg>
    </button>
  </div>
</div>

<div class="ascii-bg" aria-hidden="true">
  <pre>                                        (      )     (                                        ###
                                         )    (       )                                     ######
                                        (      )     (                                   #######
                                                                                      ########
                                                                                   ########
                                        #############                            ########
                                  #########*******#########                   ########
                              ######******         ******######            #########
                           #####****        .....        ****#####       #########
                         ####***     ......       ......     ***####  #########
                       ####**    ......    ...    ...   ....    **###########
                      ###**   ...                           ...   **######
                    ###***  ...                               ...  ***###
                   ###**   ..    ...    ...    ...    ...    .....   **###
                  ###**   ..                                     ..   **###
                 ###**   ..                                       ..   **###
                 ##**   ..    ...    ...    ...    ...    ...    ....   **##
                ###**  ..                                           ..  **###
                ###*   ..                                           ..   *###
                ##**  ..   ...    ...    ...    ...    ...    ...    ..  **##
                ##**  ..                                             ..  **##
                ##**  ..                                             ..  **##
                ###*   ....    ...    ...    ...    ...    ...    ....   *###
                ###**  ..                                           ..  **###
                 ##**   ..                                         ..   **##
                 ###**   .. ...    ...    ...    ...    ...    .....   **###
                  ###**   ..                                     ..   **###
                   ###**   ..                                   ..   **###
                    ###***  ... ...    ...    ...    ...    .....  ***###
                      ###**   ...                           ...   **###
                       ####**    ....                   ....    **####
                         ####***     .........    ......     ***####
                           #####****        .....        ****#####
                              ######******         ******######
                                  #########*******#########
                                        #############</pre>
</div>

{@render children()}

<Settings />
