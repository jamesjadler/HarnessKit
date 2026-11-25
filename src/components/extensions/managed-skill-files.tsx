import type { ReactNode } from "react";
import { ManagedFileRow } from "@/components/agents/managed-file-list";
import type { ManagedFile } from "@/lib/managed-files";
import { skillRelativePath, type SkillRoot } from "@/lib/managed-skill-files";
import { CreateVariantAction } from "./create-variant-action";
import type { GroupedExtension } from "@/lib/types";
import { agentDisplayName } from "@/lib/types";

export function ManagedSkillFiles({
  files,
  roots,
  group,
}: {
  files: ManagedFile[];
  roots: SkillRoot[];
  group?: GroupedExtension;
}) {
  function tree(items: ManagedFile[], prefix = ""): ReactNode {
    const dirs = new Map<string, ManagedFile[]>();
    const leaves: ManagedFile[] = [];
    for (const f of items) {
      const original = f.original_id
        ? (files.find((o) => o.id === f.original_id) ?? f)
        : f;
      const relative = skillRelativePath(original, roots).slice(prefix.length);
      const slash = relative.indexOf("/");
      if (slash < 0) leaves.push(f);
      else {
        const dir = relative.slice(0, slash);
        dirs.set(dir, [...(dirs.get(dir) ?? []), f]);
      }
    }
    return (
      <>
        {leaves.map((f) => (
          <ManagedFileRow
            key={f.id}
            file={f}
            variantExtras={
              group && f.file_name === "SKILL.md" ? (
                <CreateVariantAction
                  group={group}
                  label="Create whole-skill variant"
                />
              ) : undefined
            }
          />
        ))}
        {[...dirs]
          .sort(([a], [b]) => a.localeCompare(b))
          .map(([dir, children]) => (
            <details key={dir} className="managed-skill-directory" open>
              <summary>{dir}</summary>
              <div>{tree(children, prefix + dir + "/")}</div>
            </details>
          ))}
      </>
    );
  }
  const agents = [...new Set(files.map((f) => f.agent))];
  return (
    <div className="managed-files managed-skill-files">
      {agents.map((agent) => (
        <section key={agent}>
          {agents.length > 1 && (
            <h4 className="managed-section-title">{agentDisplayName(agent)}</h4>
          )}
          {tree(files.filter((f) => f.agent === agent))}
        </section>
      ))}
    </div>
  );
}
