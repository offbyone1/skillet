// In-app updates via the Tauri updater plugin. The check reads latest.json from
// this project's GitHub releases; signatures are verified against the public
// key in tauri.conf.json before anything is installed.
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { settings } from "./settings.svelte";

const LAST_CHECK_KEY = "skillet.update.lastCheck";
const AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
const AUTO_CHECK_DELAY_MS = 30_000;

export type UpdateKind = "idle" | "loading" | "success" | "error";

export const update = $state<{
  version: string;
  status: string;
  kind: UpdateKind;
  availableVersion: string | null;
  checking: boolean;
  installing: boolean;
}>({
  version: "",
  status: "",
  kind: "idle",
  availableVersion: null,
  checking: false,
  installing: false,
});

let available: Update | null = null;

function setStatus(status: string, kind: UpdateKind) {
  update.status = status;
  update.kind = kind;
}

function markChecked() {
  try {
    localStorage.setItem(LAST_CHECK_KEY, String(Date.now()));
  } catch {
    /* ignore */
  }
}

export async function loadVersion() {
  try {
    update.version = await getVersion();
  } catch {
    update.version = "";
  }
}

export async function checkForUpdates(manual: boolean) {
  if (update.checking || update.installing) return;
  update.checking = true;
  available = null;
  update.availableVersion = null;
  if (manual) setStatus("Checking for updates…", "loading");
  try {
    const found = await check({ timeout: 15_000 });
    if (!manual) markChecked();
    if (!found) {
      if (manual) setStatus("Skillet is up to date.", "success");
      return;
    }
    available = found;
    update.availableVersion = found.version;
    setStatus(`Version ${found.version} is available.`, "success");
  } catch (err) {
    if (!manual) markChecked();
    const message = err instanceof Error ? err.message : String(err);
    if (manual) setStatus(`Update check failed: ${message}`, "error");
    console.warn("update check failed:", err);
  } finally {
    update.checking = false;
  }
}

export async function installUpdate() {
  const found = available;
  if (!found || update.installing) return;
  update.installing = true;
  let downloaded = 0;
  try {
    setStatus("Downloading update…", "loading");
    await found.downloadAndInstall((event) => {
      if (event.event === "Started") {
        downloaded = 0;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        setStatus(`Downloading update… ${(downloaded / 1024 / 1024).toFixed(1)} MB`, "loading");
      } else if (event.event === "Finished") {
        setStatus("Installing update…", "loading");
      }
    });
    setStatus("Update installed. Restarting…", "success");
    await relaunch();
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    setStatus(`Update install failed: ${message}`, "error");
    console.warn("update install failed:", err);
    update.installing = false;
  }
}

/** Once a day, 30 s after start, when automatic checks are on. */
export function scheduleAutoUpdateCheck() {
  if (!settings.autoUpdate) return;
  let last = 0;
  try {
    last = Number(localStorage.getItem(LAST_CHECK_KEY) ?? 0);
  } catch {
    /* ignore */
  }
  if (Number.isFinite(last) && Date.now() - last < AUTO_CHECK_INTERVAL_MS) return;
  window.setTimeout(() => {
    if (settings.autoUpdate) void checkForUpdates(false);
  }, AUTO_CHECK_DELAY_MS);
}
