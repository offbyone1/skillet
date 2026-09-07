export type Scope = "personal" | "project" | "plugin";
export type Quality = "good" | "warn" | "bad";

export interface Skill {
  name: string;
  scope: Scope;
  plugin: string | null;
  description: string;
  allowed_tools: string | null;
  size_bytes: number;
  size_human: string;
  file_count: number;
  modified: string;
  modified_ts: number;
  desc_quality: Quality;
  path: string;
}

export interface McpServer {
  name: string;
  endpoint: string;
  transport: "http" | "stdio";
  status: string; // connected | needs-auth | pending | error | unknown
  managed: boolean;
  scope: string; // user | local | project | plugin | managed | —
  tools: number | null;
}

export interface Finding {
  severity: "crit" | "warn" | "info";
  title: string;
  detail: string;
  fix: string;
}

// ---- Agent-Starter (Loadout-Profile) — mirrors src-tauri/src/profiles.rs ----

export type AgentKind = "claude" | "codex";

export interface SkillRef {
  name: string;
  scope: "personal" | "project" | "plugin";
  plugin: string | null;
  path: string;
}

export interface McpRef {
  name: string;
  scope: string; // user | local | project | plugin | managed
  sourcePath: string;
}

export interface Loadout {
  id: string;
  name: string;
  note: string;
  agent: AgentKind;
  workdir: string;
  skills: SkillRef[];
  mcps: McpRef[];
  model: string | null;
  effort: string | null;
  prompt: string | null;
  skipPerms: boolean;
  builtin: boolean;
}
