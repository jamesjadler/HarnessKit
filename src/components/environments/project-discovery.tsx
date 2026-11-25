import { useRef, useState } from "react";
import { agentDisplayName } from "@/lib/types";
import { useEnvironmentStore } from "@/stores/environment-store";
import { PathInput } from "./path-picker";
export function ProjectDiscovery({
  environment,
  agent,
}: {
  environment: string;
  agent?: string;
}) {
  const command = useEnvironmentStore((s) => s.command);
  const [root, setRoot] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const inFlight = useRef(false);
  const lastSaved = useRef("");
  const submit = async (path: string) => {
    const next = path.trim();
    if (!next || inFlight.current || next === lastSaved.current) return;
    inFlight.current = true;
    setBusy(true);
    setError(null);
    setMessage("");
    try {
      await command({
        action: agent ? "add_agent_project" : "discover_projects",
        environment,
        ...(agent ? { agent } : {}),
        root: next,
      });
      lastSaved.current = next;
      setRoot("");
      setMessage(
        agent
          ? `Matching projects added to ${agentDisplayName(agent)}.`
          : "Matching projects categorized under their agents.",
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      inFlight.current = false;
    }
  };
  return (
    <div
      className="quiet-add"
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <PathInput
        label={`${agent ? agentDisplayName(agent) : "Environment"} project directory`}
        value={root}
        onChange={(v) => {
          setRoot(v);
          lastSaved.current = "";
        }}
        onSave={(path) => void submit(path)}
        environment={environment}
        placeholder="Choose a folder or paste a path…"
        disabled={busy}
        invalid={!!error}
      />
      {busy && (
        <p role="status" className="py-1 text-xs text-muted-foreground">
          Finding projects and agent files…
        </p>
      )}
      {error && (
        <p role="alert" className="py-1 text-xs text-destructive">
          {error}
        </p>
      )}
      {message && (
        <p role="status" className="py-1 text-xs text-[var(--trust-safe)]">
          {message}
        </p>
      )}
    </div>
  );
}
