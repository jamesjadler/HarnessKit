import { type ReactNode, useEffect, useRef, useState } from "react";
import { api } from "@/lib/invoke";
import { useAgentStore } from "@/stores/agent-store";
import { toast } from "@/stores/toast-store";

const drafts = new Map<string, string>();
const pending = new Map<string, Promise<void>>();
const requested = new Map<string, { value: string; input: HTMLInputElement }>();
async function savePath(agent: string) {
  if (pending.has(agent)) return;
  const next = requested.get(agent);
  if (!next) return;
  requested.delete(agent);
  const { value, input } = next;
  const job = api
    .updateAgentPath(agent, value)
    .then(() => {
      if (drafts.get(agent)?.trim() === value) drafts.delete(agent);
      input.dataset.savedPath = value;
      useAgentStore.setState((s) => ({
        agents: s.agents.map((a) =>
          a.name === agent ? { ...a, path: value } : a,
        ),
      }));
    })
    .catch((e) => toast.error(`${agent}: path was not saved. ${String(e)}`))
    .finally(() => {
      pending.delete(agent);
      if (requested.has(agent)) void savePath(agent);
    });
  pending.set(agent, job);
  await job;
}
export function AgentPathInput({
  agent,
  path,
  disabled,
}: {
  agent: string;
  path: string;
  disabled: boolean;
}) {
  const [value, setValue] = useState(drafts.get(agent) ?? path);
  useEffect(() => {
    if (!drafts.has(agent)) setValue(path);
  }, [agent, path]);
  return (
    <input
      data-agent-path={agent}
      data-saved-path={path}
      aria-label={`${agent} config path`}
      disabled={disabled}
      value={value}
      onChange={(e) => {
        drafts.set(agent, e.target.value);
        setValue(e.target.value);
      }}
      className="min-w-0 flex-1 rounded-md border border-border bg-background px-3 py-1 text-sm disabled:opacity-40"
    />
  );
}
export function AutoSaveAgentPaths({ children }: { children: ReactNode }) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const node = root.current;
    const save = () => {
      for (const input of node?.querySelectorAll<HTMLInputElement>(
        "input[data-agent-path]",
      ) ?? []) {
        const agent = input.dataset.agentPath ?? "";
        const value = input.value.trim();
        if (!drafts.has(agent)) continue;
        if (value === input.dataset.savedPath && !pending.has(agent)) {
          drafts.delete(agent);
          continue;
        }
        if (!value) {
          toast.error(`${agent}: path cannot be empty`);
          continue;
        }
        requested.set(agent, { value, input });
        void savePath(agent);
      }
    };
    const blur = (e: FocusEvent) => {
      if (!node?.contains(e.relatedTarget as Node | null)) save();
    };
    node?.addEventListener("focusout", blur);
    return () => {
      node?.removeEventListener("focusout", blur);
      save();
    };
  }, []);
  return <div ref={root}>{children}</div>;
}
