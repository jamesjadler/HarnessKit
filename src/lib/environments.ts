import type { Extension } from "./types";
export interface PathAccess {
  state: "available" | "empty" | "missing" | "denied" | "error" | "unchecked";
  count: number | null;
  error: string | null;
}
export interface AgentProject {
  tracked?: boolean;
  enabled?: boolean;
  access?: PathAccess;
  id: string;
  environment: string;
  agent: string;
  root: string;
}
export interface AgentCatalogEntry {
  tracked?: boolean;
  kind?: "skills" | "instructions" | "config" | "hooks";
  is_directory?: boolean;
  id?: string;
  scope?: string;
  project?: string | null;
  access?: PathAccess;
  home_access?: PathAccess;
  home_enabled?: boolean;
  environment: string;
  name: string;
  home: string;
  root: string;
  detected: boolean;
  enabled: boolean;
  provider: string | null;
  error: string | null;
}
export interface EnvironmentProvider {
  disabled?: boolean;
  last_error?: string | null;
  last_scanned?: string | null;
  scope?: string;
  id: string;
  name: string;
  root: string;
  paused: boolean;
}
export interface ManagedEnvironment {
  connection?: {
    type: "local" | "ssh" | "ssh_password" | "wsl" | "docker";
    container?: string;
    context?: string | null;
    host?: string;
    user?: string;
    port?: number;
    distribution?: string;
  };
  id: string;
  machine: string;
  name: string;
  os: string;
  paused: boolean;
  providers: EnvironmentProvider[];
}
export interface SkillMember {
  id: string;
  environment: string;
  provider: string;
  path: string;
  excluded: boolean;
  fingerprint: string | null;
  previous: string | null;
  error: string | null;
  scanned_at: string | null;
}
export interface FolderSnapshot {
  modes?: Record<string, number>;
  directories?: string[];
  files: Record<string, number[]>;
  fingerprint: string;
}
export interface SkillRevision {
  path?: string | null;
  number: number;
  environment: string;
  captured_at: string;
  snapshot: FolderSnapshot | null;
}
export interface ManagedSkill {
  id: string;
  name: string;
  scope: string;
  original: string | null;
  stopped: boolean;
  members: SkillMember[];
  revisions: SkillRevision[];
}
export interface SyncPreferences {
  automatic: boolean;
  policy: "all" | "like" | "none";
  direction: "bidirectional" | "host";
  host: string | null;
  removal: "propagate" | "ask" | "ignore";
}
export type SyncDecision =
  | {
      status:
        | "identity_decision"
        | "synced"
        | "conflict"
        | "removal_decision"
        | "off";
    }
  | { status: "change"; source: string; targets: string[] };
export interface EnvironmentOverview {
  storage_bytes?: number;
  inventory?: Extension[];
  registry: {
    catalog?: AgentCatalogEntry[];
    projects?: AgentProject[];
    jobs?: {
      id: string;
      environment: string;
      provider: string;
      name: string;
      source: string;
      status: string;
      error: string | null;
    }[];
    version: number;
    generation: number;
    environments: ManagedEnvironment[];
    skills: ManagedSkill[];
    preferences: SyncPreferences;
  };
  statuses: { id: string; decision: SyncDecision }[];
}
export function memberStatus(
  member: SkillMember,
  skill: ManagedSkill,
  decision?: SyncDecision,
): "off" | "error" | "attention" | "synced" {
  if (skill.stopped || member.excluded || decision?.status === "off")
    return "off";
  if (member.error) return "error";
  if (
    decision?.status === "identity_decision" ||
    decision?.status === "conflict" ||
    decision?.status === "removal_decision" ||
    decision?.status === "change"
  )
    return "attention";
  return "synced";
}
