import { Check, Clock, Monitor, TriangleAlert, Unplug } from "lucide-react";
import { Link } from "react-router-dom";
import { memberStatus } from "@/lib/environments";
import type { GroupedExtension } from "@/lib/types";
import { useEnvironmentStore } from "@/stores/environment-store";
import { EnvironmentConnectionHelp } from "./connection-guide";
export function EnvironmentIcons({ group }: { group: GroupedExtension }) {
  const data = useEnvironmentStore((s) => s.data);
  const skill = data?.registry.skills.find((s) =>
    group.instances.some((i) => i.sync_skill_id === s.id),
  );
  const envs = data?.registry.environments ?? [];
  if (!skill)
    return (
      <span
        className="inline-flex"
        title="Local environment · not tracked for synchronization"
      >
        <Monitor size={16} className="text-muted-foreground" />
      </span>
    );
  const decision = data?.statuses.find((s) => s.id === skill.id)?.decision;
  const visible = skill.members.filter((m) =>
    group.instances.some((i) => i.sync_member_id === m.id),
  );
  return (
    <span className="inline-flex gap-2">
      {[...new Set(visible.map((m) => m.environment))].map((id) => {
        const e = envs.find((e) => e.id === id);
        const members = visible.filter((m) => m.environment === id);
        const states = members.map((m) => memberStatus(m, skill, decision));
        const state = states.includes("error")
          ? "error"
          : states.includes("attention")
            ? "attention"
            : states.every((s) => s === "off")
              ? "off"
              : "synced";
        const label = `${e?.machine} / ${e?.name} · ${state === "off" ? "Sync off by choice" : state === "error" ? members.find((m) => m.error)?.error : state === "attention" ? "Needs attention or update queued" : "Synced"}`;
        return (
          <span
            key={id}
            title={label}
            role="img"
            aria-label={label}
            className={
              state === "off"
                ? "opacity-40 text-muted-foreground"
                : state === "error"
                  ? "text-destructive"
                  : state === "attention"
                    ? "text-warning"
                    : "text-[var(--trust-safe)]"
            }
          >
            {e?.os === "windows" ? (
              <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="currentColor"
                aria-hidden="true"
              >
                <path d="M3 3h8v8H3zm10 0h8v8h-8zM3 13h8v8H3zm10 0h8v8h-8z" />
              </svg>
            ) : (
              <Monitor size={18} />
            )}
          </span>
        );
      })}
    </span>
  );
}
export function SyncStatus({ group }: { group: GroupedExtension }) {
  const data = useEnvironmentStore((s) => s.data);
  const skill = data?.registry.skills.find((s) =>
    group.instances.some((i) => i.sync_skill_id === s.id),
  );
  const d = data?.statuses.find((s) => s.id === skill?.id)?.decision;
  const members =
    skill?.members.filter((m) =>
      group.instances.some((i) => i.sync_member_id === m.id),
    ) ?? [];
  const unavailable = members.find((m) => m.error);
  const paused = members.some((m) =>
    data?.registry.environments.some(
      (e) =>
        e.id === m.environment &&
        (e.paused ||
          e.providers.some(
            (p) => p.id === m.provider && (p.paused || p.disabled),
          )),
    ),
  );
  if (!d || d.status === "off" || group.instances.every((i) => i.sync_excluded))
    return (
      <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
        <Unplug size={13} />
        Not synced
      </span>
    );
  if (unavailable) {
    const environment = data?.registry.environments.find(
      (e) => e.id === unavailable.environment,
    );
    if (environment)
      return (
        <EnvironmentConnectionHelp
          environment={environment}
          error={unavailable.error}
          label="Unavailable · how to fix"
        />
      );
  }
  if (paused)
    return <span className="text-xs text-warning">Ⅱ Sync paused</span>;
  if (d.status === "synced")
    return (
      <span className="inline-flex items-center gap-1 text-xs text-[var(--trust-safe)]">
        <Check size={13} />
        Synced
      </span>
    );
  return (
    <Link
      onClick={(e) => e.stopPropagation()}
      to={`/notifications?skill=${skill?.id}`}
      className="inline-flex items-center gap-1 rounded bg-warning/10 px-2 py-1 text-xs text-warning"
    >
      {d.status === "change" ? (
        <Clock size={13} />
      ) : (
        <TriangleAlert size={13} />
      )}{" "}
      {d.status === "change"
        ? "Update queued"
        : "Conflict · user attention needed"}
    </Link>
  );
}
export function EnvironmentSelector() {
  const { data, filter, setFilter } = useEnvironmentStore();
  return (
    <select
      aria-label="Environment"
      value={filter}
      onChange={(e) => setFilter(e.target.value)}
      className="max-w-full rounded-lg border border-border bg-background px-2 py-1 text-xs"
    >
      <option value="all">All environments</option>
      {data?.registry.environments.map((e) => (
        <option key={e.id} value={e.id}>
          {e.machine} / {e.name}
        </option>
      ))}
    </select>
  );
}
export function ProviderSyncControl({ provider }: { provider: string }) {
  const { data, busy, command } = useEnvironmentStore();
  const env = data?.registry.environments.find((e) => e.id === "local");
  const roots = env?.providers.filter((p) => p.name === provider) ?? [];
  if (!roots.length)
    return <span className="text-xs text-muted-foreground">Not tracked</span>;
  const paused = env?.paused || roots.some((p) => p.paused || p.disabled);
  return (
    <button
      disabled={busy || env?.paused || roots.some((p) => p.disabled)}
      title={
        env?.paused
          ? "Environment sync is paused"
          : `${paused ? "Resume" : "Pause"} ${provider} sync`
      }
      onClick={() => {
        Promise.all(
          roots.map((p) =>
            command({
              action: "pause",
              environment: "local",
              provider: p.id,
              paused: !paused,
            }),
          ),
        ).catch(() => {});
      }}
      className={`shrink-0 text-xs ${paused ? "text-warning" : "text-[var(--trust-safe)]"}`}
    >
      {paused ? "▶ Paused" : "Ⅱ Active"}
    </button>
  );
}
