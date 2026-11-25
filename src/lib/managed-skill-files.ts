import type { ManagedFile } from "./managed-files";
export const isSkillFile = (file: ManagedFile) =>
  file.category === "skills" || file.category === "skill";
export interface SkillRoot {
  agent: string;
  path: string;
}
export const skillDirectory = (path: string) =>
  path
    .replace(/\\/g, "/")
    .replace(/\/SKILL\.md(?:\.disabled)?$/i, "")
    .replace(/\/$/, "");
export function selectSkillFiles(files: ManagedFile[], roots: SkillRoot[]) {
  const originals = files.filter(
    (f) =>
      isSkillFile(f) &&
      f.copies.some((c) =>
        roots.some(
          (r) =>
            f.agent === r.agent &&
            c.path.replace(/\\/g, "/").startsWith(skillDirectory(r.path) + "/"),
        ),
      ),
  );
  const ids = new Set(originals.map((f) => f.original_id ?? f.id));
  return files.filter(
    (f) =>
      originals.includes(f) ||
      (f.original_id !== null && ids.has(f.original_id)),
  );
}
export function skillRelativePath(file: ManagedFile, roots: SkillRoot[]) {
  for (const copy of file.copies)
    for (const root of roots) {
      const prefix = skillDirectory(root.path) + "/",
        path = copy.path.replace(/\\/g, "/");
      if (file.agent === root.agent && path.startsWith(prefix))
        return path.slice(prefix.length);
    }
  return file.file_name;
}
