import { Link } from "react-router-dom";
import { useEnvironmentStore } from "@/stores/environment-store";
export function RemoteSkillFiles({ id }: { id: string }) {
  const data = useEnvironmentStore((s) => s.data);
  const skill = data?.registry.skills.find((s) =>
    s.members.some((m) => `environment:${m.id}` === id),
  );
  const member = skill?.members.find((m) => `environment:${m.id}` === id);
  const snapshot = skill?.revisions
    .slice()
    .reverse()
    .find((r) => r.snapshot?.fingerprint === member?.fingerprint)?.snapshot;
  if (!skill || !member || !snapshot)
    return (
      <p className="text-xs text-muted-foreground">
        No cached files available.
      </p>
    );
  return (
    <section className="space-y-2 rounded border border-border p-3 text-xs">
      <p className="break-all font-mono">{member.path}</p>
      <p className={member.error ? "text-warning" : "text-muted-foreground"}>
        Snapshot captured {member.scanned_at ?? "at last scan"}
        {member.error ? ` · ${member.error}` : ""}
      </p>
      <Link
        className="inline-block underline"
        to={`/notifications?skill=${skill.id}`}
      >
        Compare or edit environment versions
      </Link>
      {Object.entries(snapshot.files)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([path, bytes]) => {
          let text: string | null = null;
          try {
            text = new TextDecoder("utf-8", { fatal: true }).decode(
              new Uint8Array(bytes),
            );
            if (text.includes("\0")) text = null;
          } catch {}
          return (
            <details key={path} className="border-t border-border py-2">
              <summary className="cursor-pointer break-all font-mono">
                {path} · {bytes.length} bytes
              </summary>
              {text === null ? (
                <p className="py-2 text-muted-foreground">
                  Binary file · retained in this revision
                </p>
              ) : (
                <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words py-2">
                  {text}
                </pre>
              )}
            </details>
          );
        })}
    </section>
  );
}
