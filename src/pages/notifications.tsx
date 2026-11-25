import { useState } from "react";
import { useSearchParams } from "react-router-dom";
import { EnvironmentConnectionHelp } from "@/components/environments/connection-guide";
import {
  type ComparisonSide,
  TextComparison,
} from "@/components/shared/text-comparison";
import {
  directoryRows,
  fileStates,
  snapshotFor,
} from "@/lib/directory-comparison";
import type {
  ManagedSkill,
  SkillMember,
  SyncDecision,
} from "@/lib/environments";
import { useEnvironmentStore } from "@/stores/environment-store";

const button =
  "rounded-lg border border-border px-3 py-2 text-xs hover:bg-accent disabled:opacity-40";
function ReviewSkill({
  skill,
  decision,
}: {
  skill: ManagedSkill;
  decision: SyncDecision;
}) {
  const { data, command, busy: storeBusy } = useEnvironmentStore();
  const [applying, setApplying] = useState(false);
  const busy = storeBusy || applying;
  const availableMembers = skill.members.filter(
    (m) => !m.error && m.fingerprint,
  );
  const initialMembers =
    availableMembers.length >= 2 ? availableMembers : skill.members;
  const [review, setReview] = useState(false),
    [left, setLeft] = useState(initialMembers[0]?.id ?? ""),
    [right, setRight] = useState(
      initialMembers[1]?.id ?? initialMembers[0]?.id ?? "",
    ),
    [third, setThird] = useState(""),
    [file, setFile] = useState("SKILL.md"),
    [ignored, setIgnored] = useState<string | null>(null);
  const [variantName, setVariantName] = useState("");
  const [dirty, setDirty] = useState(false);
  const discard = () =>
    !busy && (!dirty || window.confirm("Discard unsaved comparison edits?"));
  const env = (m: SkillMember) =>
    data?.registry.environments.find((e) => e.id === m.environment);
  const revision = (m: SkillMember) =>
    [...skill.revisions]
      .reverse()
      .find(
        (r) =>
          r.snapshot?.fingerprint === m.fingerprint &&
          r.environment === m.environment,
      );
  const snapshot = (m: SkillMember) => snapshotFor(skill, m);
  const family = data?.registry.skills.filter(
    (s) =>
      s.id === skill.id ||
      s.id === skill.original ||
      s.original === (skill.original ?? skill.id),
  ) ?? [skill];
  const historical = (id: string) => {
    if (!id.startsWith("revision:")) return null;
    const [, skillId, number] = id.split(":");
    const owner = family.find((s) => s.id === skillId);
    const revision = owner?.revisions.find((r) => r.number === Number(number));
    return owner && revision ? { owner, revision } : null;
  };
  const files = [
    ...new Set([
      ...skill.members.flatMap((m) => Object.keys(snapshot(m)?.files ?? {})),
      ...Object.keys(historical(left)?.revision.snapshot?.files ?? {}),
      ...Object.keys(historical(right)?.revision.snapshot?.files ?? {}),
      ...Object.keys(historical(third)?.revision.snapshot?.files ?? {}),
    ]),
  ].sort();
  const name = (m: SkillMember) =>
    `${env(m)?.name ?? m.environment} · ${m.fingerprint ? `r${revision(m)?.number ?? "?"}` : "removed"}`;
  const keep = (m: SkillMember) =>
    command({
      action: "keep",
      skill: skill.id,
      member: m.id,
      generation: data?.registry.generation,
    }).catch(() => {});
  const toSide = (id: string): ComparisonSide | null => {
    const history = historical(id);
    if (history) {
      const bytes = history.revision.snapshot?.files[file];
      if (!bytes) return null;
      try {
        const content = new TextDecoder("utf-8", { fatal: true }).decode(
          new Uint8Array(bytes),
        );
        if (content.includes("\0")) return null;
        return {
          id,
          environment: history.owner.name,
          provider: "History",
          path: history.revision.path
            ? `${history.revision.path}/${file}`
            : `${file} (original location not recorded)`,
          revision: String(history.revision.number),
          revisionName: `Revision ${history.revision.number} · ${data?.registry.environments.find((e) => e.id === history.revision.environment)?.name ?? history.revision.environment}`,
          content,
          capturedAt: history.revision.captured_at,
          readOnly: true,
        };
      } catch {
        return null;
      }
    }
    const m = skill.members.find((m) => m.id === id);
    if (!m) return null;
    const bytes = snapshot(m)?.files[file];
    if (!bytes) return null;
    try {
      const content = new TextDecoder("utf-8", { fatal: true }).decode(
        new Uint8Array(bytes),
      );
      if (content.includes("\0")) return null;
      return {
        id: m.id,
        environment: env(m)?.name ?? m.environment,
        provider: m.provider,
        path: `${m.path}/${file}`,
        revision: m.fingerprint ?? "",
        revisionName: `${m.error ? "Cached · " : ""}Revision ${revision(m)?.number ?? "unknown"}`,
        readOnly: !!m.error,
        content,
        capturedAt: m.scanned_at ?? undefined,
      };
    } catch {
      return null;
    }
  };
  const a = toSide(left),
    b = toSide(right);
  return (
    <section className="space-y-3 rounded-xl border border-border bg-card p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h2 className="font-semibold">{skill.name}</h2>
          <p
            className={`text-xs ${decision.status === "synced" ? "text-[var(--trust-safe)]" : "text-warning"}`}
          >
            {decision.status === "identity_decision"
              ? "Same name · confirm identity"
              : decision.status === "conflict"
                ? "Conflict · user attention needed"
                : decision.status === "removal_decision"
                  ? "Removed outside HarnessKit"
                  : decision.status === "change"
                    ? "Update ready / queued"
                    : decision.status === "off"
                      ? "Sync off by choice"
                      : "Synced"}
          </p>
        </div>
        <button
          disabled={busy}
          className={button}
          onClick={() => {
            if (discard()) {
              setDirty(false);
              setReview((v) => !v);
            }
          }}
        >
          {review ? "Close review" : "Review"}
        </button>
      </div>
      {decision.status === "identity_decision" && (
        <div className="rounded border border-warning/40 p-3 text-xs">
          <p>
            These installations share a folder name, but their common origin is
            unverified. Review paths and contents before linking them.
          </p>
          <button
            className={button}
            disabled={busy}
            onClick={() =>
              command({ action: "confirm_identity", skill: skill.id }).catch(
                () => {},
              )
            }
          >
            Treat these as the same skill
          </button>
        </div>
      )}
      <div className="flex flex-wrap gap-2">
        {skill.members
          .filter((m) => !m.excluded)
          .map((m) => (
            <button
              key={m.id}
              className={button}
              disabled={busy || !!m.error || skill.stopped}
              title={m.error ?? m.path}
              onClick={() => keep(m)}
            >
              {m.fingerprint ? "Keep" : "Sync removal from"} {name(m)}
            </button>
          ))}
        <button
          disabled={busy}
          className={button}
          onClick={() =>
            command({
              action: "stop",
              skill: skill.id,
              stopped: !skill.stopped,
            }).catch(() => {})
          }
        >
          {skill.stopped
            ? "Resume syncing this skill"
            : "Stop syncing this skill"}
        </button>
      </div>
      {skill.members
        .filter((m) => !m.fingerprint && m.previous && !m.error && !m.excluded)
        .map((m) => (
          <div key={m.id} className="flex flex-wrap items-center gap-2 text-xs">
            <span>{env(m)?.name}: confirmed removal</span>
            <button
              className={button}
              disabled={busy}
              onClick={async () => {
                try {
                  await command({
                    action: "ignore",
                    skill: skill.id,
                    member: m.id,
                    always: false,
                  });
                  setIgnored(m.id);
                } catch {}
              }}
            >
              Ignore
            </button>
          </div>
        ))}
      {ignored && (
        <div className="rounded border border-border p-3 text-sm">
          Removal acknowledged. Always ignore external skill removals?
          <div className="mt-2 flex gap-2">
            <button className={button} onClick={() => setIgnored(null)}>
              Just this time
            </button>
            <button
              className={button}
              onClick={async () => {
                try {
                  await command({
                    action: "preferences",
                    preferences: {
                      ...data?.registry.preferences,
                      removal: "ignore",
                    },
                  });
                  setIgnored(null);
                } catch {}
              }}
            >
              Always ignore this type
            </button>
          </div>
          <p className="mt-2 text-xs text-muted-foreground">
            Change this later in Settings. Other environments keep their copies.
          </p>
        </div>
      )}
      {review && (
        <>
          <details className="rounded border border-border p-3">
            <summary className="cursor-pointer text-sm">
              Revision history · {skill.revisions.length} revisions
            </summary>
            <button
              className={`${button} mt-2`}
              disabled={busy || skill.revisions.length <= 10}
              onClick={() => {
                if (
                  window.confirm(
                    "Permanently prune older revisions? Current copies, sync baselines, and the newest 10 revisions will be retained.",
                  )
                )
                  command({ action: "prune", skill: skill.id, keep: 10 }).catch(
                    () => {},
                  );
              }}
            >
              Prune older revisions
            </button>
            <label className="my-2 block text-xs">
              New variant name{" "}
              <input
                className="ml-2 rounded border border-border bg-background p-1"
                value={variantName}
                onChange={(e) => setVariantName(e.target.value)}
              />
            </label>
            <p className="text-xs text-muted-foreground">
              Revert creates a new revision in the first selected environment;
              it follows the sync policy. Variants have independent identities.
            </p>
            {[...skill.revisions].reverse().map((r) => (
              <div
                key={r.number}
                className="flex flex-wrap items-center gap-2 border-b border-border py-2 text-xs"
              >
                <span>
                  r{r.number} ·{" "}
                  {
                    data?.registry.environments.find(
                      (e) => e.id === r.environment,
                    )?.name
                  }{" "}
                  · {r.captured_at}
                  {r.snapshot ? "" : " · removed"}
                </span>
                <button
                  disabled={busy}
                  className={button}
                  onClick={() =>
                    command({
                      action: "revision",
                      skill: skill.id,
                      member: left,
                      number: r.number,
                      variant: null,
                    }).catch(() => {})
                  }
                >
                  Revert
                </button>
                <button
                  disabled={busy || !variantName.trim() || !r.snapshot}
                  className={button}
                  onClick={() =>
                    command({
                      action: "revision",
                      skill: skill.id,
                      member: left,
                      number: r.number,
                      variant: variantName.trim(),
                    }).catch(() => {})
                  }
                >
                  Create variant
                </button>
              </div>
            ))}
          </details>
          <div className="overflow-auto">
            <table className="w-full text-xs">
              <thead>
                <tr>
                  <th className="p-2 text-left">File</th>
                  {skill.members.map((m) => (
                    <th className="min-w-40 p-2 text-left" key={m.id}>
                      {name(m)}
                      <p className="max-w-60 break-all font-mono font-normal text-muted-foreground">
                        {m.path}
                      </p>
                      {m.error && (
                        <>
                          <p className="text-destructive">{m.error}</p>
                          {data?.registry.environments
                            .filter((e) => e.id === m.environment)
                            .map((env) => (
                              <EnvironmentConnectionHelp
                                key={env.id}
                                environment={env}
                                error={m.error}
                              />
                            ))}
                        </>
                      )}
                      <button
                        disabled={busy}
                        className="mt-2 text-xs font-normal underline"
                        onClick={() =>
                          command({
                            action: "participation",
                            skill: skill.id,
                            member: m.id,
                            enabled: m.excluded,
                          }).catch(() => {})
                        }
                      >
                        {m.excluded
                          ? "Include in sync"
                          : "Stop syncing this copy"}
                      </button>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {directoryRows(files).map((node) => {
                  const states = fileStates(skill, node.path);
                  return (
                    <tr key={node.path}>
                      <td
                        className="p-2"
                        style={{ paddingLeft: `${8 + node.depth * 16}px` }}
                      >
                        {node.directory ? (
                          <span className="font-mono">▾ {node.name}/</span>
                        ) : (
                          <button
                            disabled={busy}
                            className="text-left font-mono underline"
                            onClick={() => {
                              if (discard()) {
                                setDirty(false);
                                setFile(node.path);
                              }
                            }}
                          >
                            └ {node.name}
                          </button>
                        )}
                      </td>
                      {skill.members.map((m) => {
                        const state = states.find((s) => s.id === m.id)?.state;
                        return (
                          <td
                            className={`p-2 ${state === "Conflict" ? "bg-warning/10 text-warning" : state === "Removed" ? "text-destructive" : state === "Added" ? "text-[var(--trust-safe)]" : "text-muted-foreground"}`}
                            key={m.id}
                          >
                            {node.directory ? "" : state}
                          </td>
                        );
                      })}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <div className="flex flex-wrap gap-2">
            {[
              [left, setLeft, "First version"],
              [right, setRight, "Second version"],
              [third, setThird, "Third version (optional)"],
            ].map(([value, setter, label]) => (
              <label className="text-xs" key={String(label)}>
                {String(label)}{" "}
                <select
                  disabled={busy}
                  className="rounded border border-border bg-background p-2"
                  value={String(value)}
                  onChange={(e) => {
                    if (discard()) {
                      setDirty(false);
                      (setter as (id: string) => void)(e.target.value);
                    }
                  }}
                >
                  {label === "Third version (optional)" && (
                    <option value="">None</option>
                  )}
                  {skill.members.map((m) => (
                    <option key={m.id} value={m.id}>
                      {name(m)}
                    </option>
                  ))}
                  {family.map((owner) => (
                    <optgroup
                      key={owner.id}
                      label={`${owner.name}${owner.id === skill.original ? " · Original" : ""} · history`}
                    >
                      {[...owner.revisions]
                        .reverse()
                        .filter((r) => r.snapshot)
                        .map((r) => (
                          <option
                            key={r.number}
                            value={`revision:${owner.id}:${r.number}`}
                          >
                            Revision {r.number} ·{" "}
                            {data?.registry.environments.find(
                              (e) => e.id === r.environment,
                            )?.name ?? r.environment}
                          </option>
                        ))}
                    </optgroup>
                  ))}
                </select>
              </label>
            ))}
          </div>
          {a && b && (!third || toSide(third)) ? (
            <TextComparison
              onDirtyChange={setDirty}
              onBusyChange={setApplying}
              key={`${left}-${right}-${third}-${file}`}
              left={a}
              right={b}
              third={third ? (toSide(third) ?? undefined) : undefined}
              onSave={async (side, content) => {
                await command({
                  action: "edit",
                  skill: skill.id,
                  member: side.id,
                  file,
                  expected: side.revision,
                  content,
                });
                const member = useEnvironmentStore
                  .getState()
                  .data?.registry.skills.find((s) => s.id === skill.id)
                  ?.members.find((m) => m.id === side.id);
                return {
                  ...side,
                  content,
                  revision: member?.fingerprint ?? side.revision,
                };
              }}
              onStopSync={() =>
                command({ action: "stop", skill: skill.id, stopped: true })
              }
            />
          ) : (
            <p className="text-xs text-muted-foreground">
              {file}: missing or binary content. Review the directory versions
              above before keeping an environment.
            </p>
          )}
        </>
      )}
    </section>
  );
}
export default function NotificationsPage() {
  const { data, error, busy, command } = useEnvironmentStore();
  const [params] = useSearchParams();
  const [showAll, setShowAll] = useState(false);
  const selected = params.get("skill");
  const pathIssues =
    data?.registry.environments.flatMap((environment) =>
      environment.providers
        .filter(
          (provider) =>
            provider.last_error &&
            !provider.disabled &&
            !data?.registry.catalog?.some(
              (c) =>
                c.environment === environment.id &&
                c.provider === provider.id &&
                (c.tracked === false || !c.enabled || c.home_enabled === false),
            ),
        )
        .map((provider) => ({ environment, provider })),
    ) ?? [];
  const jobs =
    data?.registry.jobs?.filter(
      (j) => showAll || j.status === "pending" || !!j.error,
    ) ?? [];
  const skills =
    data?.registry.skills.filter(
      (s) =>
        showAll ||
        s.id === selected ||
        data.statuses.some(
          (d) =>
            d.id === s.id &&
            d.decision.status !== "synced" &&
            d.decision.status !== "off",
        ),
    ) ?? [];
  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold">Notifications</h1>
        <button
          className={button}
          disabled={busy}
          onClick={() => command({ action: "refresh" }).catch(() => {})}
        >
          Refresh / preview sync
        </button>
      </header>
      {error && (
        <p role="alert" className="mb-3 text-destructive">
          {error}
        </p>
      )}
      <label className="mb-3 text-xs">
        <input
          type="checkbox"
          checked={showAll}
          onChange={(e) => setShowAll(e.target.checked)}
        />{" "}
        Include synced and intentionally unsynced skills
      </label>
      <div className="min-h-0 space-y-4 overflow-auto">
        {!data && !error && (
          <p className="text-sm text-muted-foreground">
            Loading notifications…
          </p>
        )}
        {pathIssues.map(({ environment, provider }) => (
          <section
            key={`${environment.id}:${provider.id}`}
            className="rounded-xl border border-yellow-500/30 bg-card p-4 text-sm"
          >
            <h2 className="font-semibold text-yellow-500">
              {environment.name} · {provider.name} · Unable to access directory
            </h2>
            <p className="mt-1 break-all font-mono text-xs">{provider.root}</p>
            <p className="mt-2 text-xs">{provider.last_error}</p>
            <EnvironmentConnectionHelp
              environment={environment}
              error={provider.last_error}
              label="Path troubleshooting"
            />
          </section>
        ))}
        {jobs.map((job) => (
          <section
            key={job.id}
            className="rounded-xl border border-border bg-card p-4 text-sm"
          >
            <b>
              {job.name} ·{" "}
              {job.status === "pending" ? "Install queued" : job.status}
            </b>
            <p className="text-xs text-muted-foreground">
              {
                data?.registry.environments.find(
                  (e) => e.id === job.environment,
                )?.name
              }{" "}
              · {job.provider} · {job.source}
            </p>
            {job.error && (
              <p className="mt-1 text-xs text-warning">{job.error}</p>
            )}
            {job.status === "pending" && (
              <div className="mt-2 flex gap-2">
                <button
                  disabled={busy}
                  className={button}
                  onClick={() =>
                    command({ action: "deliver_installs" }).catch(() => {})
                  }
                >
                  Retry delivery
                </button>
                <button
                  disabled={busy}
                  className={button}
                  onClick={() =>
                    command({ action: "cancel_install", job: job.id }).catch(
                      () => {},
                    )
                  }
                >
                  Cancel install
                </button>
              </div>
            )}
          </section>
        ))}
        {skills.map((skill) => (
          <ReviewSkill
            key={skill.id}
            skill={skill}
            decision={
              data?.statuses.find((d) => d.id === skill.id)?.decision ?? {
                status: "off",
              }
            }
          />
        ))}
        {data &&
          !error &&
          !skills.length &&
          !jobs.length &&
          !pathIssues.length && (
            <p className="text-sm text-muted-foreground">
              No changes require your attention.
            </p>
          )}
      </div>
    </div>
  );
}
