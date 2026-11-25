import { Info, RefreshCw, TriangleAlert, X } from "lucide-react";
import { useState } from "react";
import type { ManagedEnvironment } from "@/lib/environments";
import { useEnvironmentStore } from "@/stores/environment-store";

export type ConnectionKind =
  | "tailscale"
  | "ssh"
  | "windows"
  | "wsl"
  | "local"
  | "docker";
export function environmentKind(env: ManagedEnvironment): ConnectionKind {
  if (env.connection?.type === "docker") return "docker";
  if (env.id === "windows-host") return "windows";
  if (env.connection?.type === "wsl") return "wsl";
  if (env.connection?.type === "ssh" || env.connection?.type === "ssh_password")
    return /\.ts\.net\.?$/.test(env.connection.host ?? "")
      ? "tailscale"
      : "ssh";
  return "local";
}
export function connectionIssue(error?: string | null) {
  if (!error) return null;
  if (/host key|REMOTE HOST IDENTIFICATION/i.test(error))
    return {
      title: "Verify the host identity",
      next: "From the environment running HarnessKit, verify the destination fingerprint with your SSH client. If a trusted key changed, check why before updating trust.",
    };
  if (/publickey|authentication|permission denied.*password/i.test(error))
    return {
      title: "SSH sign-in needs setup",
      next: "Check the remote account and existing SSH alias first. Make its authorized key or SSH agent available to the HarnessKit process; sign-in must work without a prompt.",
    };
  if (/connection refused/i.test(error))
    return {
      title: "SSH service is not accepting connections",
      next: "Check the port and enable the destination SSH service. On a Mac: System Settings → General → Sharing → Remote Login; allow the intended account.",
    };
  if (/python3.*(not found|not recognized|no such)|no.*python3/i.test(error))
    return {
      title: "Python 3 is unavailable",
      next: "Install Python 3 on the destination and make python3 available to this account in a noninteractive session.",
    };
  if (/wsl stopped|distribution.*stopped/i.test(error))
    return {
      title: "Start the WSL distribution",
      next: "Use Start distribution below, then check again. Discovery never starts stopped distributions automatically.",
    };
  if (/timed out|timeout|unreachable|resolve hostname|offline/i.test(error))
    return {
      title: "Check network reachability",
      next: "Check the hostname, power, network route and firewall. For Tailscale, check both devices and access rules. Keeping a computer awake does not enable SSH.",
    };
  if (/no such file|not found|directory|folder|permission denied/i.test(error))
    return {
      title: "Check folder access",
      next: "Check the provider path, mount and account permissions. An unavailable folder is not treated as an intentional deletion.",
    };
  return {
    title: "Connection needs attention",
    next: "Check the details below, then retry. No successful checks are assumed from this error.",
  };
}
const guides: Record<
  ConnectionKind,
  { title: string; intro: string; steps: [string, string][] }
> = {
  tailscale: {
    title: "SSH over Tailscale",
    intro:
      "Try the connection first. Tailscale supplies the network route; SSH still needs a service and sign-in method.",
    steps: [
      [
        "Connect the network",
        "Connect both computers to the tailnet and allow the SSH port through access rules and the destination firewall.",
      ],
      [
        "Enable SSH on the destination",
        "On macOS: System Settings → General → Sharing → Remote Login, then allow your account. On Linux, enable its OpenSSH server.",
      ],
      [
        "Reuse SSH authentication",
        "Use an existing SSH alias, key or agent available to the process running HarnessKit. Verify the host fingerprint on the first connection.",
      ],
      [
        "Check Python and folders",
        "The destination needs python3 and access to the provider folders. Current SSH execution expects a Unix-like shell; native Windows SSH is not supported by this helper.",
      ],
      [
        "Review and enable sync",
        "After connection succeeds, review detected paths and sync direction. New connections start paused.",
      ],
    ],
  },
  docker: {
    title: "Docker / Dev Containers",
    intro:
      "Use the Docker context already available to HarnessKit. No SSH service inside the container is needed.",
    steps: [
      [
        "Choose the runtime",
        "Make the Docker CLI and daemon accessible to the account running HarnessKit. Leave context blank to reuse the current Docker context.",
      ],
      [
        "Select a running container",
        "Discover containers or enter a container name. Discovery never starts stopped containers; start one in your container manager if needed.",
      ],
      [
        "Check Python and agent homes",
        "The container needs Python 3 and readable agent folders for its configured user. Provider folders on bind mounts or volumes are blocked to prevent syncing the same underlying files; use their existing host mapping instead.",
      ],
      [
        "Review and enable",
        "Review detected paths and sync direction before resuming the initially paused connection. Recreated containers require a new connection check.",
      ],
    ],
  },
  ssh: {
    title: "Direct SSH",
    intro:
      "Use an existing SSH alias or a reachable hostname. Leave account and port blank to reuse SSH configuration.",
    steps: [
      [
        "Choose a reachable host",
        "Enter a LAN/VPN hostname, IP or SSH alias. Tailscale is optional.",
      ],
      [
        "Enable the SSH service",
        "Check the destination SSH server and firewall port. On a Mac, enable Remote Login and allow the intended account.",
      ],
      [
        "Verify sign-in",
        "Verify the host fingerprint and use a key or agent available to HarnessKit. Authentication must finish without prompting.",
      ],
      [
        "Check Python and folders",
        "Check python3 and provider folder permissions. The remote helper requires a Unix-like shell.",
      ],
      [
        "Review and enable sync",
        "Review detected folders and direction before resuming the initially paused connection.",
      ],
    ],
  },
  windows: {
    title: "Windows host from WSL",
    intro:
      "Windows profile discovery is automatic. No SSH, Tailscale or Windows Python is needed.",
    steps: [
      [
        "Check Windows access",
        "WSL needs Windows interoperability and the Windows drive mounted.",
      ],
      [
        "Use the detected profile",
        "HarnessKit reads the Windows profile and its saved provider paths. C:/Users/demon and /mnt/c/Users/demon refer to the same files.",
      ],
      [
        "Review paths",
        "If discovery cannot read the profile, map an accessible mounted folder. Windows and WSL remain separate environments.",
      ],
      [
        "Enable selected sync",
        "Newly detected Windows access starts paused. Review the folders, then resume sync when ready.",
      ],
    ],
  },
  wsl: {
    title: "WSL on this Windows host",
    intro:
      "Discover installed distributions first. No SSH or Tailscale is required.",
    steps: [
      [
        "Choose the distribution",
        "HarnessKit uses wsl.exe on the same Windows machine. Windows-side discovery registers installed distributions.",
      ],
      [
        "Start only when needed",
        "Start a stopped distribution explicitly. Discovery does not start it for you.",
      ],
      [
        "Check Python and account",
        "The distribution’s default user needs python3 and access to the provider folders.",
      ],
      [
        "Review paths and sync",
        "Windows drive paths and /mnt/<drive> aliases point to the same drive folders. Review detected paths, then enable sync.",
      ],
    ],
  },
  local: {
    title: "Local or mounted folder",
    intro:
      "No SSH or Python installation is required. Use a folder already accessible to HarnessKit.",
    steps: [
      [
        "Make the folder accessible",
        "Mount the disk or share in the environment running HarnessKit. HarnessKit does not manage SMB/NFS credentials or create mounts.",
      ],
      [
        "Select the skills directory",
        "Use an absolute path. Local Windows drive-letter and WSL mounted-drive spellings are resolved for the running platform.",
      ],
      [
        "Check permissions",
        "The account running HarnessKit needs read access; syncing also needs permission to replace files and directories.",
      ],
      [
        "Review and enable sync",
        "Confirm the environment, provider and sync policy. An unavailable mount is not treated as deletion.",
      ],
    ],
  },
};
export function ConnectionGuide({
  kind,
  context,
  error,
  onRetry,
  onStart,
  busy = false,
  onClose,
}: {
  kind: ConnectionKind;
  context: string;
  error?: string | null;
  onRetry?: () => void;
  onStart?: () => void;
  busy?: boolean;
  onClose?: () => void;
}) {
  const runtime = useEnvironmentStore((s) =>
    s.data?.registry.environments.find((e) => e.id === "local"),
  );
  const issue = connectionIssue(error);
  const guide = guides[kind];
  const ssh = kind === "ssh" || kind === "tailscale";
  return (
    <section
      aria-label={`Connection setup: ${context}`}
      className="my-3 min-w-0 max-w-2xl space-y-3 [&_code]:break-all rounded-xl border border-primary/40 border-l-4 border-l-primary bg-primary/5 p-4 text-sm"
    >
      <header className="flex items-start justify-between gap-3 border-b border-primary/20 pb-3">
        <div>
          <p className="text-xs font-bold uppercase tracking-wider text-primary">
            Connection setup
          </p>
          <p className="text-xs text-muted-foreground">{context}</p>
          <h3 className="mt-1 text-lg font-semibold">{guide.title}</h3>
        </div>
        {onClose && (
          <button
            type="button"
            aria-label="Close connection help"
            onClick={onClose}
            className="rounded p-1 hover:bg-accent"
          >
            <X size={16} />
          </button>
        )}
      </header>
      {issue ? (
        <div
          className="rounded-lg border border-warning/50 bg-warning/10 p-3"
          role="status"
        >
          <p className="flex items-center gap-2 font-semibold">
            <TriangleAlert size={16} className="shrink-0 text-warning" />
            {issue.title}
          </p>
          <p className="mt-1 text-sm">{issue.next}</p>
        </div>
      ) : (
        <p className="flex items-start gap-2">
          <Info size={16} className="mt-1 shrink-0 text-primary" />
          {guide.intro}
        </p>
      )}
      <p className="text-xs text-muted-foreground">
        Run checks where HarnessKit runs
        {runtime ? `: ${runtime.name} on ${runtime.machine}` : ""}, using that
        process’s account. Browser location does not change file or SSH access.
      </p>
      <details>
        <summary className="cursor-pointer font-semibold text-primary">
          Setup steps in order
        </summary>
        <ol className="mt-3 space-y-2">
          {guide.steps.map(([title, body], i) => (
            <li
              key={title}
              className="flex gap-3 rounded-lg border border-border bg-card p-3"
            >
              <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary">
                {i + 1}
              </span>
              <div>
                <h4 className="font-semibold">{title}</h4>
                <p className="text-xs leading-relaxed text-muted-foreground">
                  {body}
                </p>
              </div>
            </li>
          ))}
        </ol>
      </details>
      {ssh && (
        <details>
          <summary className="cursor-pointer font-semibold">
            Need an SSH key?
          </summary>
          <ol className="mt-2 list-decimal space-y-2 pl-5 text-xs leading-relaxed">
            <li>
              Check for an existing key first. If needed, run{" "}
              <code>ssh-keygen -t ed25519</code> in the environment running
              HarnessKit, choosing an unused filename.
            </li>
            <li>
              Add only the public .pub contents to the destination account’s{" "}
              <code>~/.ssh/authorized_keys</code>, on that machine or through a
              password-authenticated session. Keep the private key private.
            </li>
            <li>
              Verify the host fingerprint with your SSH client. Load a
              passphrase-protected key into an SSH agent available to
              HarnessKit.
            </li>
            <li>
              Run <code>ssh -o BatchMode=yes user@host python3 --version</code>{" "}
              with your account and host (or alias). It must finish without
              prompting. Start HarnessKit from that same authenticated
              environment.
            </li>
          </ol>
        </details>
      )}
      {kind === "tailscale" && (
        <p className="text-xs text-muted-foreground">
          This uses ordinary SSH over Tailscale. The separate Tailscale SSH
          feature has its own platform and policy requirements.
        </p>
      )}
      {error && (
        <details>
          <summary className="cursor-pointer text-xs text-muted-foreground">
            Technical details
          </summary>
          <pre className="mt-2 whitespace-pre-wrap break-all text-xs">
            {error}
          </pre>
        </details>
      )}
      {(onRetry || onStart) && (
        <div className="flex flex-wrap gap-2">
          {onRetry && (
            <button
              type="button"
              disabled={busy}
              onClick={onRetry}
              className="inline-flex items-center gap-2 rounded-lg bg-primary px-3 py-2 text-xs text-primary-foreground disabled:opacity-50"
            >
              <RefreshCw size={13} />
              {busy ? "Checking…" : "Check again"}
            </button>
          )}
          {onStart && (
            <button
              type="button"
              disabled={busy}
              onClick={onStart}
              className="rounded-lg border border-warning/50 px-3 py-2 text-xs text-warning disabled:opacity-50"
            >
              Start distribution
            </button>
          )}
        </div>
      )}
    </section>
  );
}
export function EnvironmentConnectionHelp({
  environment,
  error,
  label = "How to fix",
}: {
  environment: ManagedEnvironment;
  error?: string | null;
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const { busy, command } = useEnvironmentStore();
  return (
    <div onClick={(e) => e.stopPropagation()}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="text-xs text-primary underline underline-offset-2"
      >
        {label}
      </button>
      {open && (
        <ConnectionGuide
          kind={environmentKind(environment)}
          context={`${environment.machine} / ${environment.name}`}
          error={error}
          busy={busy}
          onClose={() => setOpen(false)}
          onRetry={() => void command({ action: "refresh" }).catch(() => {})}
          onStart={
            environment.connection?.type === "wsl"
              ? () =>
                  void command({
                    action: "start",
                    environment: environment.id,
                  }).catch(() => {})
              : undefined
          }
        />
      )}
    </div>
  );
}
