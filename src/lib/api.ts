import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Skill, McpServer, Finding, Loadout } from "./types";

export const scanSkills = () => invoke<Skill[]>("scan_skills");
export const listMcp = () => invoke<McpServer[]>("list_mcp");
export const healthCheck = () => invoke<Finding[]>("health_check");
export const claudeVersion = () => invoke<string>("claude_version");

// ---- Agent-Starter ----
export const listProfiles = () => invoke<Loadout[]>("list_profiles");
export const saveProfile = (profile: Loadout) => invoke<void>("save_profile", { profile });
export const deleteProfile = (id: string) => invoke<void>("delete_profile", { id });
export const preflightWorkdir = (workdir: string) =>
  invoke<string[]>("preflight_workdir", { workdir });
export const launchAgent = (id: string, workdirOverride: string | null, confirmed: boolean) =>
  invoke<string>("launch_agent", { id, workdirOverride, confirmed });

/** Native folder picker (Tauri dialog). Returns the chosen path or null. */
export const pickFolder = async (defaultPath?: string) => {
  const res = await open({ directory: true, multiple: false, defaultPath: defaultPath || undefined });
  return typeof res === "string" ? res : null;
};
