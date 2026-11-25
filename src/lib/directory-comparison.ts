import type { FolderSnapshot, ManagedSkill, SkillMember } from "./environments";
export function snapshotFor(
  skill: ManagedSkill,
  member: SkillMember,
  previous = false,
): FolderSnapshot | null {
  const fingerprint = previous ? member.previous : member.fingerprint;
  if (!fingerprint) return null;
  return (
    [...skill.revisions]
      .reverse()
      .find((r) => r.snapshot?.fingerprint === fingerprint)?.snapshot ?? null
  );
}
const bytesEqual = (a: number[] | undefined, b: number[] | undefined) =>
  a === b ||
  (!!a && !!b && a.length === b.length && a.every((v, i) => v === b[i]));
export function fileStates(skill: ManagedSkill, path: string) {
  const copies = skill.members.map((member) => ({
    member,
    current: snapshotFor(skill, member)?.files[path],
    previous: snapshotFor(skill, member, true)?.files[path],
  }));
  const changed = copies.filter(
    (c) =>
      !c.member.excluded &&
      !c.member.error &&
      !bytesEqual(c.current, c.previous),
  );
  const conflict = changed.some((c, i) =>
    changed.slice(i + 1).some((other) => !bytesEqual(c.current, other.current)),
  );
  return copies.map(({ member, current, previous }) => ({
    id: member.id,
    state: member.error
      ? "Cached"
      : conflict && !member.excluded
        ? "Conflict"
        : !current
          ? previous
            ? "Removed"
            : "Absent"
          : !previous
            ? "Added"
            : bytesEqual(current, previous)
              ? "Unchanged"
              : "Changed",
  }));
}
export function directoryRows(
  paths: string[],
): { path: string; name: string; depth: number; directory: boolean }[] {
  const nodes = new Map<string, boolean>();
  for (const path of paths) {
    const parts = path.split("/");
    parts.forEach((_, i) => {
      nodes.set(parts.slice(0, i + 1).join("/"), i < parts.length - 1);
    });
  }
  return [...nodes]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, directory]) => ({
      path,
      name: path.split("/").pop() ?? path,
      depth: path.split("/").length - 1,
      directory,
    }));
}
