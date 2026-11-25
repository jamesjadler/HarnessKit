import type { AgentDetail } from "./types";
export interface FileRevision {
  number: number;
  content: string;
  fingerprint: string;
  created_at: string;
  reason: string;
}
export interface ManagedFileCopy {
  id: string;
  environment_id: string;
  environment: string;
  machine: string;
  path: string;
  provider_id: string;
  content: string | null;
  fingerprint: string | null;
  accepted_fingerprint: string | null;
  error: string | null;
  enabled: boolean;
  pending: boolean;
}
export interface ManagedFile {
  id: string;
  agent: string;
  category: string;
  scope: string;
  scope_path: string | null;
  logical_path: string;
  file_name: string;
  variant_name: string | null;
  original_id: string | null;
  revisions: FileRevision[];
  copies: ManagedFileCopy[];
}
export interface ManagedFiles {
  files: ManagedFile[];
}
export interface ManagedFileRequest {
  action: "list" | "save" | "accept" | "restore" | "variant" | "retry";
  file_id?: string;
  copy_id?: string;
  expected?: string;
  content?: string;
  revision?: number;
  name?: string;
  observed?: Record<string, string>;
}
export const latestRevision = (file: ManagedFile) =>
  file.revisions.reduce<FileRevision | undefined>(
    (best, r) => (!best || r.number > best.number ? r : best),
    undefined,
  );
export function managedStatus(
  file: ManagedFile,
):
  | "Synced"
  | "Not-synced"
  | "Partial"
  | "Unavailable"
  | "Not assigned"
  | "Sync off" {
  if (!file.copies.length) return "Not assigned";
  const enabled = file.copies.filter((c) => c.enabled);
  if (!enabled.length) return "Sync off";
  const healthy = enabled.filter((c) => !c.error && c.fingerprint !== null);
  if (!healthy.length) return "Unavailable";
  if (healthy.length !== enabled.length) return "Partial";
  const revision = latestRevision(file);
  return revision &&
    healthy.every((c) => !c.pending && c.fingerprint === revision.fingerprint)
    ? "Synced"
    : "Not-synced";
}
export function withManagedAgents(
  details: AgentDetail[],
  files: ManagedFile[],
): AgentDetail[] {
  const names = new Set(files.map((f) => f.agent));
  return [
    ...details.map((a) => (names.has(a.name) ? { ...a, detected: true } : a)),
    ...[...names]
      .filter((n) => !details.some((a) => a.name === n))
      .map((name) => ({
        name,
        detected: true,
        config_files: [],
        extension_counts: { skill: 0, mcp: 0, plugin: 0, hook: 0, cli: 0 },
      })),
  ];
}
