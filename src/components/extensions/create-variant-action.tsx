import { Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { VariantNameDialog } from "@/components/shared/variant-name-dialog";
import { latestRevision } from "@/lib/managed-files";
import type { GroupedExtension } from "@/lib/types";
import { useEnvironmentStore } from "@/stores/environment-store";
import { useExtensionStore } from "@/stores/extension-store";
import { useManagedFileStore } from "@/stores/managed-file-store";
const normalize = (path: string) => path.replace(/\\/g, "/").replace(/\/$/, "");
export function CreateVariantAction({ group }: { group: GroupedExtension }) {
  const [open, setOpen] = useState(false),
    [notice, setNotice] = useState("");
  const data = useEnvironmentStore((s) => s.data),
    environmentCommand = useEnvironmentStore((s) => s.command),
    environmentBusy = useEnvironmentStore((s) => s.busy);
  const files = useManagedFileStore((s) => s.files),
    refresh = useManagedFileStore((s) => s.refresh),
    command = useManagedFileStore((s) => s.command),
    fileBusy = useManagedFileStore((s) => s.busy);
  useEffect(() => {
    void refresh();
  }, [refresh]);
  const skill = data?.registry.skills.find((s) =>
    group.instances.some(
      (i) =>
        i.sync_skill_id === s.id ||
        s.members.some(
          (m) =>
            normalize(m.path) ===
            normalize(i.source_path ?? "").replace(
              /\/SKILL\.md(\.disabled)?$/,
              "",
            ),
        ),
    ),
  );
  const member = skill?.members.find(
    (m) => !m.error && m.fingerprint && !m.excluded,
  );
  const revision = skill?.revisions
    .filter((r) => r.snapshot && r.snapshot.fingerprint === member?.fingerprint)
    .sort((a, b) => b.number - a.number)[0];
  const file = files.find(
    (f) =>
      group.agents.includes(f.agent) &&
      f.copies.some((c) =>
        group.instances.some(
          (i) => normalize(i.source_path ?? "") === normalize(c.path),
        ),
      ),
  );
  const supported =
    !!(skill && member && revision) || !!(file && latestRevision(file));
  return (
    <div className="mt-3 flex flex-wrap items-center gap-3">
      <button
        className="inline-flex items-center gap-1 text-xs text-primary disabled:opacity-40"
        disabled={!supported || environmentBusy || fileBusy}
        title={
          supported
            ? "Create an independent named duplicate"
            : "A tracked file or skill snapshot is required to create a variant"
        }
        onClick={() => setOpen(true)}
      >
        <Plus size={13} /> Create variant
      </button>
      {notice && (
        <span role="status" className="text-xs text-muted-foreground">
          {notice}
        </span>
      )}
      {open && (
        <VariantNameDialog
          source={group.name}
          onClose={() => setOpen(false)}
          onCreate={async (name) => {
            if (skill && member && revision) {
              await environmentCommand({
                action: "revision",
                skill: skill.id,
                member: member.id,
                number: revision.number,
                variant: name,
              });
              await useExtensionStore.getState().fetch();
            } else if (file) {
              await command({
                action: "variant",
                file_id: file.id,
                expected: latestRevision(file)?.fingerprint,
                name,
              });
            } else throw new Error("No saved version is available");
            setNotice(
              `Created ${name} with an independent identity and version history.`,
            );
          }}
        />
      )}
    </div>
  );
}
