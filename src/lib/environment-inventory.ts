import type { EnvironmentOverview } from "./environments";
import type { Extension } from "./types";
export function mergeEnvironmentInventory(
  local: Extension[],
  overview: EnvironmentOverview | null,
): Extension[] {
  if (!overview) return local;
  const normalize = (path: string) =>
    path
      .replaceAll("\\", "/")
      .replace(/\/SKILL\.md(?:\.disabled)?$/, "")
      .replace(/\/$/, "");
  const result = local
    .filter((e) => !e.id.startsWith("environment:"))
    .map((extension) => {
      if (extension.kind !== "skill" || !extension.source_path)
        return extension;
      const sourcePath = normalize(extension.source_path);
      for (const skill of overview.registry.skills) {
        const member = skill.members.find(
          (m) =>
            normalize(m.path) === sourcePath &&
            overview.registry.environments.some(
              (env) =>
                env.id === m.environment &&
                env.providers.some(
                  (p) =>
                    p.id === m.provider && extension.agents.includes(p.name),
                ),
            ),
        );
        if (member) {
          if (member.scanned_at && !member.fingerprint && !member.error)
            return null;
          const cached = overview.inventory?.find(
            (e) => e.sync_member_id === member.id,
          );
          return {
            ...extension,
            description: cached?.description ?? extension.description,
            permissions: cached?.permissions ?? extension.permissions,
            enabled: cached?.enabled ?? extension.enabled,
            environment_id: member.environment,
            sync_skill_id: skill.id,
            sync_member_id: member.id,
            sync_revision: member.fingerprint,
            sync_excluded:
              skill.stopped ||
              member.excluded ||
              overview.registry.preferences.policy === "none",
          };
        }
      }
      return extension;
    });
  const present = result.filter((e): e is Extension => e !== null);
  const members = new Set(present.map((e) => e.sync_member_id));
  return [
    ...present,
    ...(overview.inventory ?? []).filter((e) => !members.has(e.sync_member_id)),
  ];
}
