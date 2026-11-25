import { AlertTriangle, Check, Pause, Play, Trash2 } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import type { PathAccess } from "@/lib/environments";
import { useEnvironmentStore } from "@/stores/environment-store";
import { PathInput } from "./path-picker";

export function accessLabel(access?: PathAccess, home = false): string {
  if (!access || access.state === "unchecked") return "Not checked yet";
  if (access.state === "missing") return "No such file or directory";
  if (access.state === "denied") return "Permission denied";
  if (access.state === "error")
    return access.error || "Unable to access directory";
  if (home) return "Accessible";
  return access.count
    ? `${access.count} skill${access.count === 1 ? "" : "s"} found`
    : "No files found";
}
export function accessColor(access?: PathAccess) {
  return access?.state === "missing"
    ? "text-yellow-500"
    : access?.state === "denied" || access?.state === "error"
      ? "text-destructive"
      : access?.state === "available"
        ? "text-[var(--trust-safe)]"
        : "text-muted-foreground";
}
export function SyncControl({
  paused,
  disabled,
  onClick,
  label,
}: {
  paused: boolean;
  disabled?: boolean;
  onClick: () => void;
  label: string;
}) {
  const action = `${paused ? "Resume" : "Pause"} ${label} syncing`;
  return (
    <button
      type="button"
      aria-label={action}
      title={action}
      disabled={disabled}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onClick();
      }}
      className={`inline-flex shrink-0 rounded border border-border p-1.5 disabled:opacity-40 ${paused ? "text-yellow-500" : "text-[var(--trust-safe)]"}`}
    >
      {paused ? <Play size={14} /> : <Pause size={14} />}
    </button>
  );
}
const drafts = new Map<string, { value: string; expected: string }>();
export function PathEntry({
  entryKey,
  label,
  path,
  enabled,
  access,
  home = false,
  onEnable,
  onSave,
  action,
  environment,
  directory = true,
  inline = false,
  onRemove,
}: {
  entryKey: string;
  label: string;
  path: string;
  enabled: boolean;
  access?: PathAccess;
  home?: boolean;
  onEnable: (enabled: boolean) => Promise<void>;
  onSave: (path: string, expected: string) => Promise<string | undefined>;
  action?: ReactNode;
  environment?: string;
  directory?: boolean;
  inline?: boolean;
  onRemove?: () => Promise<void>;
}) {
  const busy = useEnvironmentStore((s) => s.busy);
  const [value, setValue] = useState(drafts.get(entryKey)?.value ?? path);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const state = useRef({
    value,
    expected: drafts.get(entryKey)?.expected ?? path,
  });
  state.current.value = value;
  const queue = useRef(Promise.resolve());
  const removing = useRef(false);
  const flush = useRef(() => {});
  flush.current = () => {
    queue.current = queue.current.then(async () => {
      if (removing.current) return;
      const next = state.current.value.trim();
      if (!drafts.has(entryKey) || next === state.current.expected) return;
      if (!next) {
        setError("Enter a path, or use Remove to forget this entry.");
        return;
      }
      setSaving(true);
      setError(null);
      try {
        const saved = await onSave(next, state.current.expected);
        state.current.expected = saved ?? next;
        if (drafts.get(entryKey)?.value.trim() === next) {
          drafts.delete(entryKey);
          state.current.value = saved ?? next;
          setValue(saved ?? next);
        } else {
          const draft = drafts.get(entryKey);
          if (draft) draft.expected = saved ?? next;
        }
      } catch (e) {
        setError(String(e));
      } finally {
        setSaving(false);
      }
    });
  };
  useEffect(() => {
    if (!drafts.has(entryKey)) {
      state.current.expected = path;
      setValue(path);
    }
  }, [entryKey, path]);
  useEffect(() => {
    const save = () => flush.current();
    window.addEventListener("harnesskit-save-provider-paths", save);
    return () => {
      window.removeEventListener("harnesskit-save-provider-paths", save);
      save();
    };
  }, []);
  const change = (next: string) => {
    drafts.set(entryKey, { value: next, expected: state.current.expected });
    state.current.value = next;
    setValue(next);
  };
  return (
    <div className={`quiet-entry ${inline ? "quiet-entry-inline" : ""}`}>
      <button
        type="button"
        aria-label={`Enable ${label}`}
        aria-pressed={enabled}
        disabled={busy}
        onClick={async (e) => {
          e.preventDefault();
          setError(null);
          try {
            await onEnable(!enabled);
          } catch (e) {
            setError(String(e));
          }
        }}
        className={`quiet-toggle ${enabled ? "" : "off"}`}
        title={enabled ? "Enabled" : "Disabled"}
      >
        <i aria-hidden="true" />
        <span>{enabled ? "Enabled" : "Disabled"}</span>
      </button>
      <span className="quiet-label">{label}</span>
      <PathInput
        label={`${label} path`}
        value={value}
        onChange={change}
        onSave={(next) => {
          change(next);
          flush.current();
        }}
        environment={environment}
        directory={directory}
        invalid={!!error}
      />
      <span
        title={access?.error ?? accessLabel(access, home)}
        className={`quiet-status ${accessColor(access)}`}
      >
        {access?.state === "available" ? (
          <Check size={13} />
        ) : ["missing", "denied", "error"].includes(access?.state ?? "") ? (
          <AlertTriangle size={13} />
        ) : null}
        <span className={accessColor(access)}>
          {saving
            ? "Saving and checking…"
            : !directory && access?.state === "available"
              ? "File found"
              : accessLabel(access, home)}
        </span>
      </span>
      <div className="quiet-actions">
        {action}
        {onRemove && (
          <button
            type="button"
            className="quiet-tool quiet-remove"
            aria-label={`Remove ${label}`}
            title="Remove entry; keep files"
            disabled={busy || saving}
            onClick={async (e) => {
              e.preventDefault();
              removing.current = true;
              setError(null);
              try {
                await onRemove();
                drafts.delete(entryKey);
                state.current.value = state.current.expected;
              } catch (e) {
                removing.current = false;
                setError(String(e));
              }
            }}
          >
            <Trash2 size={14} />
          </button>
        )}
      </div>
      {error && (
        <p role="alert" className="quiet-error text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
