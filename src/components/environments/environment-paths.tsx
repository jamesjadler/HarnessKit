import { ChevronRight } from "lucide-react";
import { type ReactNode, useState } from "react";
import { Link } from "react-router-dom";

import type {
  AgentCatalogEntry,
  AgentProject,
  ManagedEnvironment,
  PathAccess,
} from "@/lib/environments";

import { agentDisplayName } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useEnvironmentStore } from "@/stores/environment-store";
import { useUIStore } from "@/stores/ui-store";
import { PathEntry, SyncControl } from "./path-entry";
import { ProjectDiscovery } from "./project-discovery";
import "./quiet-tree.css";
export type LocalPaths = ReactNode | ((names: string[]) => ReactNode);
export function errorAccess(error?: string | null): PathAccess | undefined {
  if (!error) return undefined;
  return {
    state: /permission denied|access.*denied|os error 13/i.test(error)
      ? "denied"
      : /no such file|not found|missing|os error 2/i.test(error)
        ? "missing"
        : "error",
    count: null,
    error,
  };
}
export function isTracked(
  c: AgentCatalogEntry,
  env: ManagedEnvironment,
  projects: AgentProject[] = [],
) {
  return (
    c.enabled &&
    c.home_enabled !== false &&
    (c.tracked ?? !!c.provider) &&
    !env.providers.some((p) => p.id === c.provider && p.disabled) &&
    !projects.some((p) => p.id === c.project && (p.enabled === false || p.tracked === false))
  );
}
export function environmentHealth(
  env: ManagedEnvironment,
  catalog: AgentCatalogEntry[] = [],
  projects: AgentProject[] = [],
) {
  const all = catalog.filter((c) => c.environment === env.id);
  const entries = all.filter((c) => isTracked(c, env, projects));
  const states = entries.map((c) => c.access ?? errorAccess(c.error));
  // Only include legacy providers absent from the catalog; an empty tracked set is not a fallback signal.
  for (const p of env.providers.filter(
    (p) => !p.disabled && !all.some((c) => c.provider === p.id),
  ))
    states.push(
      errorAccess(p.last_error) ??
        (p.last_scanned
          ? { state: "available", count: null, error: null }
          : undefined),
    );
  for (const p of projects.filter(
    (p) =>
      p.environment === env.id &&
      p.enabled !== false && p.tracked !== false &&
      !all.some((c) => c.name === p.agent && c.home_enabled === false),
  )) {
    if (
      p.access &&
      ["missing", "denied", "error"].includes(p.access.state) &&
      !entries.some(
        (c) =>
          c.project === p.id &&
          ["missing", "denied", "error"].includes(c.access?.state ?? ""),
      )
    )
      states.push(p.access);
  }
  const issues = states.filter(
    (s) => s && ["missing", "denied", "error"].includes(s.state),
  ).length;
  const available = states.filter(
    (s) => s && ["available", "empty"].includes(s.state),
  ).length;
  return {
    issues,
    available,
    label: issues
      ? `Partial · ${issues} path issue${issues === 1 ? "" : "s"}`
      : states.some((s) => !s || s.state === "unchecked")
        ? "Checking paths…"
        : "Synced",
  };
}
const button =
  "rounded border border-border px-2 py-1.5 text-xs hover:bg-accent disabled:opacity-40";
function AgentPaths({
  env,
  agent,
  entries,
}: {
  env: ManagedEnvironment;
  agent: string;
  entries: AgentCatalogEntry[];
}) {
  const { data, busy, command } = useEnvironmentStore();
  const [error, setError] = useState<string | null>(null);
  const projects =
    data?.registry.projects?.filter(
      (p) => p.environment === env.id && p.agent === agent,
    ) ?? [];
  const providers = env.providers.filter((p) => p.name === agent);
  const base = entries.find((c) => !c.project) ?? entries[0];
  const run = async (req: Record<string, unknown>) => {
    setError(null);
    try {
      await command(req);
    } catch (e) {
      setError(String(e));
      throw e;
    }
  };
  const save = async (
    req: Record<string, unknown>,
  ): Promise<string | undefined> => {
    await command(req);
    if (env.id === "local" && req.action === "agent_home")
      await useAgentStore.getState().fetch();
    const registry = useEnvironmentStore.getState().data?.registry;
    if (req.action === "agent_home")
      return registry?.catalog?.find(
        (c) => c.environment === env.id && c.name === agent && !c.project,
      )?.home;
    if (req.action === "project_path")
      return registry?.projects?.find((p) => p.id === req.project)?.root;
    return registry?.catalog?.find(
      (c) =>
        c.environment === env.id &&
        (req.entry ? c.id === req.entry : c.provider === req.provider),
    )?.root;
  };
  const tracked = entries.filter(
    (c) => isTracked(c, env, projects) && !c.project,
  );
  const untracked = entries.filter(
    (c) => !isTracked(c, env, projects) && !c.project,
  );
  const rows = (items: AgentCatalogEntry[]) =>
    items.map((c) => {
      const p = env.providers.find((p) => p.id === c.provider);
      const id = c.id ?? p?.id ?? c.root;
      const access = c.access ?? errorAccess(c.error ?? p?.last_error);
      const usable = access?.state === "available" || access?.state === "empty";
      return (
        <PathEntry
          key={id}
          entryKey={`${env.id}:${id}`}
          label={
            c.kind === "instructions"
              ? "Instructions"
              : c.kind === "config"
                ? "Configuration"
                : c.kind === "hooks"
                  ? "Hooks"
                  : c.project
                    ? "Project skills"
                    : "Global skills"
          }
          environment={env.id}
          directory={c.is_directory !== false}
          onRemove={() =>
            command({ action: "remove_path", environment: env.id, entry: id })
          }
          path={c.root}
          enabled={isTracked(c, env, projects)}
          access={access}
          onSave={(path, expected) =>
            save({
              action: c.id ? "catalog_path" : "provider_path",
              environment: env.id,
              ...(c.id ? { entry: c.id } : { provider: p?.id }),
              expected,
              ...(c.id ? { path } : { root: path }),
            })
          }
          onEnable={(enabled) =>
            command({
              action: "catalog_enabled",
              environment: env.id,
              entry: id,
              enabled,
            })
          }
          action={
            p
              ? usable && (
                  <SyncControl
                    label={`${agent} path`}
                    paused={p.paused || env.paused}
                    disabled={
                      busy || env.paused || !isTracked(c, env, projects)
                    }
                    onClick={() =>
                      run({
                        action: "pause",
                        environment: env.id,
                        provider: p.id,
                        paused: !p.paused,
                      }).catch(() => {})
                    }
                  />
                )
              : (!c.kind || c.kind === "skills") &&
                usable &&
                !!access?.count && (
                  <button
                    className={button}
                    disabled={busy || !c.enabled}
                    onClick={() =>
                      run({
                        action: "map_provider",
                        environment: env.id,
                        provider: agent,
                        root: c.root,
                        scope: c.scope ?? "global",
                      }).catch(() => {})
                    }
                  >
                    {isTracked(c, env, projects) ? "Enable sync" : "Track"}
                  </button>
                )
          }
        />
      );
    });
  const projectRows = (items: AgentProject[]) =>
    items.map((project) => (
      <details key={project.id} open className="quiet-project">
        <summary>
          <ChevronRight className="quiet-chevron" />
          <PathEntry
            entryKey={`${env.id}:project:${project.id}`}
            label={
              project.root.split(/[\\/]/).filter(Boolean).pop() || "Project"
            }
            path={project.root}
            enabled={project.enabled !== false}
            home
            inline
            environment={env.id}
            access={project.access}
            onRemove={() =>
              command({
                action: "remove_project",
                environment: env.id,
                project: project.id,
              })
            }
            onSave={(path, expected) =>
              save({
                action: "project_path",
                environment: env.id,
                project: project.id,
                expected,
                path,
              })
            }
            onEnable={(enabled) =>
              command({
                action: "project_enabled",
                environment: env.id,
                project: project.id,
                enabled,
              })
            }
          />
        </summary>
        <div className="quiet-components">
          {project.enabled !== false && project.tracked === false && (
            <p className="px-4 py-2 text-xs text-muted-foreground">Not tracked: the agent base path is unavailable or disabled.</p>
          )}
          {rows(entries.filter((c) => c.project === project.id))}
        </div>
      </details>
    ));
  return (
    <details open className="quiet-agent">
      <summary>
        <ChevronRight className="quiet-chevron" />
        {base && (
          <PathEntry
            entryKey={`${env.id}:${agent}:home`}
            label={agentDisplayName(agent)}
            path={base.home}
            enabled={base.home_enabled ?? base.enabled}
            access={base.home_access}
            home
            inline
            environment={env.id}
            onRemove={() =>
              command({ action: "remove_agent", environment: env.id, agent })
            }
            onSave={(path, expected) =>
              save({
                action: "agent_home",
                environment: env.id,
                agent,
                expected,
                path,
              })
            }
            onEnable={async (enabled) => {
              await command({
                action: "agent_enabled",
                environment: env.id,
                agent,
                enabled,
              });
              if (env.id === "local") await useAgentStore.getState().fetch();
            }}
            action={
              providers.length > 0 && (
                <SyncControl
                  label={agentDisplayName(agent)}
                  paused={env.paused || providers.every((p) => p.paused)}
                  disabled={busy || env.paused || base.home_enabled === false}
                  onClick={() => {
                    const paused = !providers.every((p) => p.paused);
                    for (const p of providers)
                      command({
                        action: "pause",
                        environment: env.id,
                        provider: p.id,
                        paused,
                      }).catch(() => {});
                  }}
                />
              )
            }
          />
        )}
      </summary>
      <div className="quiet-agent-body">
        {error && (
          <p role="alert" className="py-2 text-xs text-destructive">
            {error}
          </p>
        )}
        {([true, false] as const).map((tracking) => {
          const items = tracking ? tracked : untracked;
          const projectItems = projects.filter(
            (p) => (p.enabled !== false && p.tracked !== false) === tracking,
          );
          return (
            <details
              key={String(tracking)}
              open={tracking}
              className="quiet-group"
              data-path-group={tracking ? "tracked" : "untracked"}
            >
              <summary>
                <ChevronRight className="quiet-chevron" />
                <span>
                  {tracking ? "Tracked" : "Untracked"} paths ·{" "}
                  {items.length + projectItems.length}
                </span>
              </summary>
              <div className="quiet-group-body">
                {rows(items)}
                {(tracking || projectItems.length > 0) && (
                  <details open className="quiet-projects">
                    <summary>
                      <ChevronRight className="quiet-chevron" />
                      <span>Projects · {projectItems.length}</span>
                      {tracking && (
                        <ProjectDiscovery environment={env.id} agent={agent} />
                      )}
                    </summary>
                    {projectRows(projectItems)}
                  </details>
                )}
              </div>
            </details>
          );
        })}
      </div>
    </details>
  );
}

export function EnvironmentPaths({
  environment: env,
}: {
  environment: ManagedEnvironment;
  localPaths?: LocalPaths;
}) {
  const { data } = useEnvironmentStore();
  const [compare, setCompare] = useState(false);
  const visibility = useUIStore((s) => s.agentVisibility);
  const catalog =
    data?.registry.catalog?.filter((c) => c.environment === env.id) ?? [];
  const entries = [
    ...catalog,
    ...env.providers
      .filter(
        (p) =>
          !catalog.some(
            (c) =>
              c.provider === p.id || (c.name === p.name && c.root === p.root),
          ),
      )
      .map((p) => ({
        environment: env.id,
        name: p.name,
        home: p.root.replace(/[\\/][^\\/]+$/, ""),
        root: p.root,
        detected: true,
        enabled: !p.disabled,
        provider: p.id,
        error: p.last_error ?? null,
        scope: p.scope,
        access: errorAccess(p.last_error),
      })),
  ];
  const agents = [...new Set(entries.map((c) => c.name))].filter(
    (agent) =>
      visibility !== "detected" ||
      entries.some(
        (c) =>
          c.name === agent &&
          (c.detected || isTracked(c, env, data?.registry.projects)),
      ) ||
      data?.registry.projects?.some(
        (p) =>
          p.environment === env.id && p.agent === agent && p.enabled !== false && p.tracked !== false,
      ),
  );
  const healthy = (environment: string, provider: string) =>
    data?.registry.environments
      .find((e) => e.id === environment)
      ?.providers.some(
        (p) => p.id === provider && !p.last_error && !p.disabled,
      );
  const comparable =
    data?.registry.skills.filter(
      (s) =>
        s.members.some(
          (m) =>
            m.environment === env.id &&
            !m.error &&
            m.fingerprint &&
            healthy(m.environment, m.provider),
        ) &&
        s.members.filter(
          (m) =>
            !m.error && m.fingerprint && healthy(m.environment, m.provider),
        ).length > 1,
    ) ?? [];
  return (
    <div className="quiet-tree space-y-3">
      {agents.map((agent) => (
        <AgentPaths
          key={agent}
          env={env}
          agent={agent}
          entries={entries.filter((c) => c.name === agent)}
        />
      ))}
      <section className="quiet-discover">
        <div>
          <h3 className="text-sm font-semibold">
            Discover projects in {env.name}
          </h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Choose a parent folder. Matching projects go to each agent
            automatically.
          </p>
        </div>
        <ProjectDiscovery environment={env.id} />
      </section>
      <button className={button} onClick={() => setCompare((v) => !v)}>
        Compare available agents
      </button>
      {compare && (
        <div className="rounded border border-border p-3 text-xs">
          {comparable.length
            ? comparable.map((s) => (
                <Link
                  key={s.id}
                  className="block text-primary underline"
                  to={`/notifications?skill=${encodeURIComponent(s.id)}`}
                >
                  {s.name} · compare available versions
                </Link>
              ))
            : "No skill currently has two available tracked copies to compare."}
        </div>
      )}
    </div>
  );
}
