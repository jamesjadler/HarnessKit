import {
  ChevronRight,
  Info,
  Laptop,
  Monitor,
  Pencil,
  RefreshCw,
} from "lucide-react";
import { useEffect, useState } from "react";
import { transport } from "@/lib/transport";
import { parseError } from "@/lib/error-types";
import { useEnvironmentStore } from "@/stores/environment-store";
import { useProjectStore } from "@/stores/project-store";
import {
  ConnectionGuide,
  type ConnectionKind,
  EnvironmentConnectionHelp,
} from "./connection-guide";
import {
  EnvironmentPaths,
  environmentHealth,
  type LocalPaths,
} from "./environment-paths";
import { SyncControl } from "./path-entry";

const fieldDescriptions: Record<string, string> = {
  Connection:
    "How HarnessKit reaches the files: SSH connects over a network, WSL and Docker use local tools, and local paths use mounted folders.",
  Authentication:
    "Username and password saves credentials securely for reuse. SSH key / agent uses keys or an existing alias configured where HarnessKit runs.",
  Machine:
    "A display label grouping environments on the same computer. For example, Windows and WSL can share a machine label. It does not change the connection address.",
  Environment:
    "A display name for this set of agent files, such as macOS, Ubuntu, or a container. You can rename it later without changing its connection.",
  "Environment name":
    "Changes only the display name. Connections, tracked paths, versions, and sync settings keep their existing identity.",
  Host: "The target computer’s hostname or IP address, including a Tailscale address. An SSH alias works only with key / agent authentication. Do not include a URL or username.",
  "SSH user":
    "The login account on the target computer, not the account running HarnessKit. Required for password authentication; optional when supplied by SSH configuration.",
  "SSH port":
    "The target SSH server’s port. Password connections default to 22. Key / agent connections use your SSH configuration when this is blank.",
  Password:
    "The target account’s login password. Saved in the operating system’s secure credential store, or protected by Windows in WSL, for future connections. Never saved in source code or environment settings.",
  "WSL distribution":
    "The registered distribution name shown by wsl --list. It identifies the Linux environment on this Windows machine.",
  Container:
    "The name or ID of an existing Docker container. It must be running and have Python 3 installed.",
  "Docker context":
    "The Docker context used to locate the container. Leave blank to use the current context where HarnessKit runs.",
  Provider:
    "The agent whose skills live in the selected directory, such as Codex or Claude Code.",
  "Skills directory":
    "An existing skills folder accessible to the HarnessKit process. For a network share, mount it first; this field does not create a network connection.",
};
function FieldHelp({ field }: { field: string }) {
  const text = fieldDescriptions[field];
  if (!text) return null;
  return (
    <span
      tabIndex={0}
      role="img"
      aria-label={`${field}: ${text}`}
      className="group relative ml-1 inline-flex cursor-help align-middle text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-primary"
    >
      <Info size={12} />
      <span
        aria-hidden="true"
        className="pointer-events-none absolute left-0 top-full z-50 mt-1 hidden w-56 max-w-[70vw] rounded-md border border-border bg-popover p-2 text-left text-xs font-normal leading-relaxed text-popover-foreground shadow-md group-hover:block group-focus:block"
      >
        {text}
      </span>
    </span>
  );
}

const button =
  "rounded border border-border px-3 py-1.5 text-xs hover:bg-accent disabled:opacity-40";
export function EnvironmentManager({
  localPaths,
}: {
  localPaths?: LocalPaths;
} = {}) {
  const { data, busy, error, command } = useEnvironmentStore();
  const projects = useProjectStore((s) => s.projects);
  const [mapping, setMapping] = useState(false),
    [mapEnvironment, setMapEnvironment] = useState("local"),
    [mapProvider, setMapProvider] = useState("codex"),
    [mapRoot, setMapRoot] = useState(""),
    [mapScope, setMapScope] = useState("global");
  const [adding, setAdding] = useState(false),
    [kind, setKind] = useState("tailscale"),
    [machine, setMachine] = useState(""),
    [name, setName] = useState(""),
    [host, setHost] = useState(""),
    [user, setUser] = useState(""),
    [root, setRoot] = useState(""),
    [provider, setProvider] = useState("codex"),
    [port, setPort] = useState(""),
    [container, setContainer] = useState(""),
    [dockerContext, setDockerContext] = useState(""),
    [candidates, setCandidates] = useState<
      { name: string; status: string; connection: Record<string, unknown> }[]
    >([]);
  const [authentication, setAuthentication] = useState("password");
  const [password, setPassword] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testSuccess, setTestSuccess] = useState(false);
  const [hostKey, setHostKey] = useState("");
  const [renameId, setRenameId] = useState<string | null>(null);
  const [renameName, setRenameName] = useState("");
  const passwordSsh =
    ["ssh", "tailscale", "ssh-vm"].includes(kind) &&
    authentication === "password";
  useEffect(() => {
    setHostKey("");
    setTestSuccess(false);
    setConnectionError(null);
  }, [host, port, user, kind, authentication]);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [discovered, setDiscovered] = useState(false);
  const run = (req: Record<string, unknown>) => command(req).catch(() => {});
  const envs = data?.registry.environments ?? [];
  const machines = [...new Set(envs.map((e) => e.machine))];
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const prefs = data?.registry.preferences;
  return (
    <div
      className="quiet-tree space-y-3"
      onBlur={(e) => {
        if (
          !e.currentTarget.contains(e.relatedTarget as Node | null) &&
          !(
            e.relatedTarget instanceof Element &&
            e.relatedTarget.closest(
              '[role="dialog"][aria-label="Choose environment path"]',
            )
          )
        )
          window.dispatchEvent(new Event("harnesskit-save-provider-paths"));
      }}
    >
      {error && !connectionError && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {!data && (
        <button className={button} onClick={() => run({ action: "refresh" })}>
          Load environments
        </button>
      )}
      {machines.map((machine) => (
        <details key={machine} className="quiet-machine">
          <summary>
            <ChevronRight className="quiet-chevron" />
            {envs.some((e) => e.machine === machine && e.id === "local") ? (
              <Monitor size={16} />
            ) : (
              <Laptop size={16} />
            )}
            {machine}
            <span className="rounded-full bg-muted px-2 py-1 text-[10px] font-normal text-muted-foreground">
              {envs.some((e) => e.machine === machine && e.id === "local")
                ? "This machine"
                : "Remote machine"}
            </span>
            <span className="ml-auto">
              <SyncControl
                label={machine}
                paused={envs
                  .filter((e) => e.machine === machine)
                  .every((e) => e.paused)}
                disabled={busy}
                onClick={() =>
                  run({
                    action: "pause_machine",
                    machine,
                    paused: !envs
                      .filter((e) => e.machine === machine)
                      .every((e) => e.paused),
                  })
                }
              />
            </span>
          </summary>
          <div className="quiet-machine-children">
            {envs
              .filter((e) => e.machine === machine)
              .map((env) => (
                <details
                  key={env.id}
                  className="quiet-environment"
                  onToggle={(e) => {
                    if (e.currentTarget.open && !checked.has(env.id)) {
                      setChecked((prev) => new Set([...prev, env.id]));
                      run({ action: "check_paths", environment: env.id });
                    }
                  }}
                >
                  <summary>
                    <ChevronRight className="quiet-chevron" />
                    <span className="font-semibold">{env.name}</span>
                    <button
                      type="button"
                      aria-label={`Rename ${env.name}`}
                      title="Rename environment"
                      disabled={busy}
                      className="p-1 text-muted-foreground hover:text-primary"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        setRenameId(env.id);
                        setRenameName(env.name);
                        const details = e.currentTarget.closest("details");
                        if (details) details.open = true;
                      }}
                    >
                      <Pencil size={13} />
                    </button>
                    <span
                      className={`ml-auto text-xs ${environmentHealth(env, data?.registry.catalog, data?.registry.projects).issues ? "text-yellow-500" : "text-[var(--trust-safe)]"}`}
                    >
                      {environmentHealth(
                        env,
                        data?.registry.catalog,
                        data?.registry.projects,
                      ).issues
                        ? environmentHealth(
                            env,
                            data?.registry.catalog,
                            data?.registry.projects,
                          ).label
                        : env.paused
                          ? "Paused"
                          : environmentHealth(
                              env,
                              data?.registry.catalog,
                              data?.registry.projects,
                            ).label}
                    </span>
                    <span>
                      <SyncControl
                        label={env.name}
                        paused={env.paused}
                        disabled={busy}
                        onClick={() =>
                          run({
                            action: "pause",
                            environment: env.id,
                            provider: null,
                            paused: !env.paused,
                          })
                        }
                      />
                    </span>
                  </summary>
                  <div className="quiet-environment-children">
                    {renameId === env.id && (
                      <form
                        className="mb-3 flex gap-2 items-end"
                        onSubmit={async (e) => {
                          e.preventDefault();
                          try {
                            await command({
                              action: "rename",
                              environment: env.id,
                              name: renameName.trim(),
                            });
                            setRenameId(null);
                          } catch {}
                        }}
                      >
                        <label className="flex-1 text-xs">
                          Environment name{" "}
                          <FieldHelp field="Environment name" />
                          <input
                            aria-label="Environment name"
                            autoFocus
                            required
                            maxLength={120}
                            className="mt-1 block w-full rounded border border-border bg-background p-2"
                            value={renameName}
                            onChange={(e) => setRenameName(e.target.value)}
                          />
                        </label>
                        <button
                          className={button}
                          disabled={busy || !renameName.trim()}
                        >
                          Save name
                        </button>
                        <button
                          type="button"
                          className={button}
                          onClick={() => setRenameId(null)}
                        >
                          Cancel
                        </button>
                      </form>
                    )}
                    {env.providers.length === 0 && (
                      <EnvironmentConnectionHelp
                        environment={env}
                        error={
                          env.providers.find((p) => p.last_error)?.last_error
                        }
                        label="Connection setup and troubleshooting"
                      />
                    )}
                    {env.connection?.type === "wsl" &&
                      (env.providers.length === 0 ||
                        env.providers.some((p) => p.last_error)) && (
                        <button
                          disabled={busy}
                          className={button}
                          onClick={() =>
                            run({ action: "start", environment: env.id })
                          }
                        >
                          Start distribution
                        </button>
                      )}
                    {env.providers.length === 0 && (
                      <p className="text-xs text-muted-foreground">
                        No provider inventory yet. Stopped environments are not
                        started automatically.
                      </p>
                    )}
                    <EnvironmentPaths
                      environment={env}
                      localPaths={localPaths}
                    />
                  </div>
                </details>
              ))}
          </div>
        </details>
      ))}
      <div className="flex flex-wrap gap-2">
        <button
          disabled={busy}
          className={button}
          onClick={() => run({ action: "refresh" })}
        >
          <RefreshCw size={12} className="mr-1 inline" />
          Refresh inventory
        </button>
        <button
          className={button}
          onClick={async () => {
            try {
              setCandidates(await transport("discover_environments"));
              setDiscovered(true);
            } catch (e) {
              useEnvironmentStore.setState({ error: String(e) });
            }
          }}
        >
          Discover environments
        </button>
        <button
          className={button}
          onClick={() => {
            setAdding((v) => !v);
            setPassword("");
            setHostKey("");
            setConnectionError(null);
          }}
        >
          Add environment
        </button>
      </div>
      {discovered && (
        <section className="space-y-2 rounded-xl border border-primary/20 bg-primary/5 p-3">
          <h3 className="font-semibold text-sm">Discovered environments</h3>
          <p className="text-xs text-muted-foreground">
            Discovery finds devices, WSL distributions and containers. Select
            one to reuse its address or runtime and check access. Stopped
            environments stay stopped.
          </p>
          <div className="rounded-lg border border-primary/20 bg-card p-3 text-xs">
            <h4 className="mb-2 font-semibold text-primary">
              Quick setup: Tailscale
            </h4>
            <ol className="list-decimal space-y-1 pl-4 text-muted-foreground">
              <li>
                Install Tailscale on this computer and the remote computer, then
                sign both into the same tailnet.
              </li>
              <li>
                Connect both devices and allow the SSH port through your tailnet
                access rules and the destination firewall.
              </li>
              <li>
                Click Discover environments again, select the device, and test
                its connection.
              </li>
            </ol>
            <p className="mt-2 text-muted-foreground">
              Tailscale provides network access. The remote computer still needs
              SSH enabled, working authentication and Python 3. Follow the
              selected connection’s setup steps for those checks.
            </p>
            <a
              className="mt-2 inline-block text-primary underline"
              href="https://tailscale.com/docs/how-to/quickstart"
              target="_blank"
              rel="noreferrer"
            >
              Tailscale installation guide ↗
            </a>
          </div>
          {!candidates.length && (
            <p className="text-xs">
              No additional devices found. You can still add a hostname or
              mounted folder below.
            </p>
          )}
          {candidates.map((c) => (
            <button
              className={`${button} block w-full text-left`}
              key={c.name}
              onClick={() => {
                setAdding(true);
                setConnectionError(null);
                setUser(String(c.connection.user ?? ""));
                setPort("");
                setPassword("");
                setHostKey("");
                setMachine(c.name);
                setName(c.name);
                setHost(String(c.connection.host ?? ""));
                setContainer(String(c.connection.container ?? ""));
                setDockerContext(String(c.connection.context ?? ""));
                setKind(
                  c.connection.type === "ssh"
                    ? "tailscale"
                    : String(c.connection.type),
                );
              }}
            >
              <span className="font-semibold">{c.name}</span>
              <span className="ml-2 text-xs text-warning">{c.status}</span>
              <span className="ml-2 text-xs text-primary">
                Set up connection →
              </span>
            </button>
          ))}
        </section>
      )}
      {adding && (
        <form
          className="space-y-3 rounded-xl border border-border border-t-4 border-t-primary p-4"
          onSubmit={async (e) => {
            e.preventDefault();
            if (connecting) return;
            setConnecting(true);
            setConnectionError(null);
            try {
              if (
                passwordSsh &&
                !window.isSecureContext &&
                !["localhost", "127.0.0.1", "::1", "[::1]"].includes(
                  window.location.hostname,
                )
              )
                throw new Error(
                  "Open HarnessKit over HTTPS or localhost before entering a password.",
                );
              await command(
                passwordSsh
                  ? {
                      action: "connect_password",
                      machine: machine.trim() || host.trim(),
                      name: name.trim() || host.trim(),
                      host: host.trim(),
                      user: user.trim(),
                      port: Number(port) || 22,
                      password,
                      host_key: hostKey,
                    }
                  : kind === "local"
                    ? {
                        action: "add",
                        machine,
                        name,
                        os: "linux",
                        provider,
                        root,
                      }
                    : {
                        action: "connect",
                        machine:
                          machine.trim() ||
                          host.trim() ||
                          name.trim() ||
                          container.trim(),
                        name: name.trim() || host.trim() || container.trim(),
                        connection:
                          kind === "docker"
                            ? {
                                type: "docker",
                                container: container.trim(),
                                context: dockerContext.trim() || null,
                              }
                            : kind === "wsl"
                              ? { type: "wsl", distribution: name }
                              : {
                                  type: "ssh",
                                  host: host.trim(),
                                  user: user.trim(),
                                  port: Number(port),
                                },
                      },
              );
              setPassword("");
              setHostKey("");
              setAdding(false);
            } catch (e) {
              setConnectionError(
                parseError(e instanceof Error ? e.message : e).message,
              );
            } finally {
              setConnecting(false);
            }
          }}
        >
          <h3 className="text-sm font-semibold">Add a connection</h3>
          <label className="block text-xs">
            Connection <FieldHelp field="Connection" />{" "}
            <select
              aria-label="Connection"
              className="ml-2 rounded border border-border bg-background p-1"
              value={kind}
              onChange={(e) => {
                setKind(e.target.value);
                setConnectionError(null);
              }}
            >
              <option value="tailscale">SSH over Tailscale</option>
              <option value="ssh">Direct SSH / existing alias</option>
              <option value="docker">Docker / Dev Container</option>
              <option value="ssh-vm">Virtual machine via SSH</option>
              <option value="wsl">WSL on this Windows host</option>
              <option value="local">
                Accessible local / mounted directory
              </option>
            </select>
          </label>
          {["ssh", "tailscale", "ssh-vm"].includes(kind) && (
            <label className="block text-xs">
              Authentication <FieldHelp field="Authentication" />
              <select
                aria-label="Authentication"
                className="mt-1 block w-full rounded border border-border bg-background p-2"
                value={authentication}
                onChange={(e) => {
                  setAuthentication(e.target.value);
                  setPassword("");
                }}
              >
                <option value="password">Username and password</option>
                <option value="key">SSH key / agent / existing alias</option>
              </select>
            </label>
          )}
          {[
            [machine, setMachine, "Machine"],
            [
              name,
              setName,
              kind === "wsl" ? "WSL distribution" : "Environment",
            ],
            ...(["ssh", "tailscale", "ssh-vm"].includes(kind)
              ? [
                  [host, setHost, "Host"],
                  [user, setUser, "SSH user"],
                  [port, setPort, "SSH port"],
                ]
              : kind === "docker"
                ? [
                    [container, setContainer, "Container"],
                    [dockerContext, setDockerContext, "Docker context"],
                  ]
                : kind === "local"
                  ? [
                      [provider, setProvider, "Provider"],
                      [root, setRoot, "Skills directory"],
                    ]
                  : []),
          ].map(([value, setter, label]) => (
            <label key={String(label)} className="block text-xs">
              {String(label)} <FieldHelp field={String(label)} />
              <input
                aria-label={String(label)}
                required={
                  kind === "local" ||
                  (kind === "wsl" && label === "WSL distribution") ||
                  (passwordSsh && label === "SSH user") ||
                  label === "Host" ||
                  label === "Container"
                }
                placeholder={
                  label === "SSH user"
                    ? passwordSsh
                      ? "Remote account username"
                      : "Use SSH configuration"
                    : label === "SSH port"
                      ? passwordSsh
                        ? "22"
                        : "Use SSH configuration (default 22)"
                      : ["ssh", "tailscale", "ssh-vm"].includes(kind) &&
                          ["Machine", "Environment"].includes(String(label))
                        ? "Use hostname"
                        : undefined
                }
                type={label === "SSH port" ? "number" : "text"}
                min={label === "SSH port" ? 1 : undefined}
                max={label === "SSH port" ? 65535 : undefined}
                className="mt-1 block w-full rounded border border-border bg-background p-2"
                value={String(value)}
                onChange={(e) =>
                  (setter as (v: string) => void)(e.target.value)
                }
              />
            </label>
          ))}
          {passwordSsh && (
            <div className="space-y-3 rounded-lg border border-primary/25 bg-primary/5 p-3">
              <label className="block text-xs">
                Password <FieldHelp field="Password" />
                <input
                  aria-label="Password"
                  type="password"
                  autoComplete="new-password"
                  required
                  maxLength={4096}
                  className="mt-1 block w-full rounded border border-border bg-background p-2"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </label>
              <p className="text-xs text-muted-foreground">
                Saved securely on the machine running HarnessKit for automatic
                reconnection. Uses macOS Keychain, Windows Credential Manager,
                or the Linux keyring. In WSL, Windows protects the saved
                credential. Passwords are never stored in environment settings.
              </p>
              <h4 className="text-sm font-semibold">Before connecting</h4>
              <ol className="list-decimal pl-4 text-xs space-y-1">
                <li>
                  On a Mac, enable System Settings → General → Sharing → Remote
                  Login and allow this user. On Linux, enable its SSH server.
                </li>
                <li>
                  Use the device’s IP or hostname. For Tailscale, connect both
                  devices to the same tailnet and use its Tailscale address.
                </li>
                <li>
                  Keep the device awake and ensure Python 3 is installed. Enter
                  its login username and password here.
                </li>
              </ol>
            </div>
          )}
          {passwordSsh && connectionError && (
            <div
              role="alert"
              className="rounded border border-warning/40 bg-warning/5 p-3 text-xs"
            >
              {connectionError.includes("Confirm SSH host fingerprint:") ? (
                <>
                  <b>Confirm this device’s SSH identity</b>
                  <p className="my-2 break-all font-mono">
                    {connectionError
                      .split("Confirm SSH host fingerprint:")[1]
                      ?.trim()}
                  </p>
                  <p>
                    Verify this fingerprint with the target device’s
                    administrator before trusting it. Your password has not been
                    sent yet.
                  </p>
                  <button
                    type="button"
                    className={button}
                    onClick={() => {
                      setHostKey(
                        connectionError
                          .split("Confirm SSH host fingerprint:")[1]
                          ?.trim() ?? "",
                      );
                      setConnectionError(null);
                    }}
                  >
                    Trust this fingerprint
                  </button>
                </>
              ) : (
                connectionError
              )}
            </div>
          )}
          {!passwordSsh && (
            <ConnectionGuide
              kind={(kind === "ssh-vm" ? "ssh" : kind) as ConnectionKind}
              context="New connection"
              error={connectionError}
            />
          )}
          {["ssh", "tailscale", "ssh-vm"].includes(kind) && (
            <button type="button" disabled={testing || connecting} className={button}
              onClick={async () => {
                setTesting(true);
                setTestSuccess(false);
                setConnectionError(null);
                try {
                  if (passwordSsh && !window.isSecureContext && !["localhost", "127.0.0.1", "::1", "[::1]"].includes(window.location.hostname))
                    throw new Error("Open HarnessKit over HTTPS or localhost before entering a password.");
                  await transport("test_ssh_connection", { req: passwordSsh
                    ? { action: "connect_password", machine: "", name: "", host: host.trim(), user: user.trim(), port: Number(port) || 22, password, host_key: hostKey }
                    : { action: "connect", machine: "", name: "", connection: { type: "ssh", host: host.trim(), user: user.trim(), port: Number(port) } }
                  });
                  setTestSuccess(true);
                } catch (e) {
                  setConnectionError(parseError(e instanceof Error ? e.message : e).message);
                } finally { setTesting(false); }
              }}>
              {testing ? "Testing SSH…" : "Test SSH"}
            </button>
          )}
          {testSuccess && <p role="status" className="text-sm text-success">SSH login and Python check passed. Ready to add.</p>}
          <button
            disabled={connecting || testing}
            className="rounded-lg bg-primary px-3 py-2 text-xs text-primary-foreground disabled:opacity-50"
          >
            {connecting ? "Connecting and saving…" : "Test connection and add"}
          </button>
        </form>
      )}
      <button className={button} onClick={() => setMapping((v) => !v)}>
        Map provider / project directory
      </button>
      {mapping && (
        <form
          className="space-y-2 rounded border border-border p-3 text-xs"
          onSubmit={async (e) => {
            e.preventDefault();
            try {
              await command({
                action: "map_provider",
                environment: mapEnvironment,
                provider: mapProvider,
                root: mapRoot,
                scope: mapScope,
              });
              setMapping(false);
            } catch {}
          }}
        >
          <label className="block">
            Environment{" "}
            <select
              className="bg-background"
              value={mapEnvironment}
              onChange={(e) => setMapEnvironment(e.target.value)}
            >
              {envs.map((e) => (
                <option key={e.id} value={e.id}>
                  {e.machine} / {e.name}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            Provider{" "}
            <input
              required
              className="rounded border border-border bg-background p-2"
              value={mapProvider}
              onChange={(e) => setMapProvider(e.target.value)}
            />
          </label>
          <label className="block">
            Scope{" "}
            <select
              className="bg-background"
              value={mapScope}
              onChange={(e) => setMapScope(e.target.value)}
            >
              <option value="global">Global</option>
              {projects.map((p) => (
                <option key={p.path} value={p.path}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            Skills directory in this environment{" "}
            <input
              required
              className="mt-1 w-full rounded border border-border bg-background p-2"
              value={mapRoot}
              onChange={(e) => setMapRoot(e.target.value)}
            />
          </label>
          <p className="text-muted-foreground">
            Map the same project scope in each environment to share its skills.
            Global and project skills remain separate.
          </p>
          <button disabled={busy} className={button}>
            Validate and add mapping
          </button>
        </form>
      )}
      {data && (
        <section className="space-y-2 border-t border-border pt-3 text-xs">
          <h3 className="font-medium">History and backup</h3>
          <p className="text-muted-foreground">
            Coordinator storage:{" "}
            {((data.storage_bytes ?? 0) / 1024 / 1024).toFixed(2)} MiB. Skill
            revisions and removed copies are retained until you explicitly prune
            them.
          </p>
          <div className="flex gap-2">
            <button
              className={button}
              onClick={() => {
                const url = URL.createObjectURL(
                  new Blob([JSON.stringify(data.registry)], {
                    type: "application/json",
                  }),
                );
                const link = document.createElement("a");
                link.href = url;
                link.download = `harnesskit-environments-${new Date().toISOString().slice(0, 10)}.json`;
                link.click();
                setTimeout(() => URL.revokeObjectURL(url), 1000);
              }}
            >
              Export backup
            </button>
            <label className={`${button} cursor-pointer`}>
              Restore backup
              <input
                type="file"
                accept="application/json,.json"
                className="sr-only"
                disabled={busy}
                onChange={async (e) => {
                  const file = e.target.files?.[0];
                  e.target.value = "";
                  if (!file) return;
                  if (
                    !window.confirm(
                      "Replace the environment registry with this backup? Existing skill folders are unchanged. All environments will be paused, and the current registry will be backed up.",
                    )
                  )
                    return;
                  try {
                    if (file.size > 128 * 1024 * 1024)
                      throw new Error("Backup exceeds 128 MiB");
                    await command({
                      action: "restore",
                      registry: JSON.parse(await file.text()),
                    });
                  } catch (error) {
                    useEnvironmentStore.setState({ error: String(error) });
                  }
                }}
              />
            </label>
          </div>
          <p className="text-muted-foreground">
            Backups contain complete skill contents and paths. Restoring
            settings does not write files to environments; review paths before
            resuming sync.
          </p>
        </section>
      )}
      {prefs && (
        <section className="space-y-3 border-t border-border pt-4">
          <h3 className="text-sm font-medium">Synchronization</h3>
          <label className="flex justify-between text-xs">
            Default policy
            <select
              value={prefs.policy}
              className="bg-background"
              onChange={(e) =>
                run({
                  action: "preferences",
                  preferences: { ...prefs, policy: e.target.value },
                })
              }
            >
              <option value="all">All providers</option>
              <option value="like">Like providers</option>
              <option value="none">None</option>
            </select>
          </label>
          <label className="flex justify-between text-xs">
            Timing
            <select
              value={prefs.automatic ? "automatic" : "manual"}
              className="bg-background"
              onChange={(e) =>
                run({
                  action: "preferences",
                  preferences: {
                    ...prefs,
                    automatic: e.target.value === "automatic",
                  },
                })
              }
            >
              <option value="automatic">Automatic</option>
              <option value="manual">Manual</option>
            </select>
          </label>
          <label className="flex justify-between text-xs">
            Direction
            <select
              value={prefs.direction}
              className="bg-background"
              onChange={(e) =>
                run({
                  action: "preferences",
                  preferences: {
                    ...prefs,
                    direction: e.target.value,
                    host: prefs.host ?? envs[0]?.id,
                  },
                })
              }
            >
              <option value="bidirectional">Bidirectional</option>
              <option value="host">Host-driven</option>
            </select>
          </label>
          {prefs.direction === "host" && (
            <label className="flex justify-between text-xs">
              Source environment
              <select
                className="bg-background"
                value={prefs.host ?? ""}
                onChange={(e) =>
                  run({
                    action: "preferences",
                    preferences: { ...prefs, host: e.target.value },
                  })
                }
              >
                {envs.map((e) => (
                  <option key={e.id} value={e.id}>
                    {e.machine} / {e.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="flex justify-between text-xs">
            External skill removals
            <select
              className="bg-background"
              value={prefs.removal}
              onChange={(e) =>
                run({
                  action: "preferences",
                  preferences: { ...prefs, removal: e.target.value },
                })
              }
            >
              <option value="propagate">Remove and sync</option>
              <option value="ask">Ask each time</option>
              <option value="ignore">Always ignore this type</option>
            </select>
          </label>
          <p className="text-xs text-muted-foreground">
            Ignore records an intentional removal only in that environment.
            Independent edits always require review.
          </p>
        </section>
      )}
    </div>
  );
}
