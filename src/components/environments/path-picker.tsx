import { ArrowUp, FileText, Folder, FolderSearch, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Modal } from "@/components/ui/modal";
import type { EnvironmentOverview } from "@/lib/environments";
import { transport } from "@/lib/transport";

export interface BrowseResult {
  path: string;
  parent: string | null;
  entries: { name: string; path: string; is_directory: boolean }[];
  truncated: boolean;
}
export function PathPicker({
  environment,
  initialPath,
  directory = true,
  onSelect,
  onClose,
}: {
  environment: string;
  initialPath: string;
  directory?: boolean;
  onSelect: (path: string) => void;
  onClose: () => void;
}) {
  const [result, setResult] = useState<BrowseResult | null>(null);
  const [path, setPath] = useState(initialPath);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sequence = useRef(0);
  const browse = useCallback(
    async (next: string) => {
      const seq = ++sequence.current;
      setBusy(true);
      setError(null);
      try {
        const data = await transport<
          EnvironmentOverview & { browse?: BrowseResult }
        >("environment_command", {
          req: { action: "browse_paths", environment, path: next },
        });
        if (seq !== sequence.current) return;
        if (!data.browse)
          throw new Error(
            "The environment did not return a directory listing.",
          );
        setResult(data.browse);
        setPath(data.browse.path);
      } catch (e) {
        if (seq === sequence.current) setError(String(e));
      } finally {
        if (seq === sequence.current) setBusy(false);
      }
    },
    [environment],
  );
  useEffect(() => {
    void browse(initialPath);
    return () => {
      sequence.current++;
    };
  }, [browse, initialPath]);
  return (
    <Modal
      onClose={onClose}
      ariaLabel="Choose environment path"
      containerClassName="flex max-h-[85vh] w-[640px] max-w-[95vw] flex-col rounded-xl border border-border bg-background shadow-xl"
    >
      <header className="flex items-center justify-between border-b border-border p-4">
        <div>
          <h2 className="font-semibold">
            Choose {directory ? "directory" : "file"}
          </h2>
          <p className="text-xs text-muted-foreground">
            Browsing {environment} · no files are changed
          </p>
        </div>
        <button type="button" onClick={onClose} aria-label="Close path picker">
          <X size={18} />
        </button>
      </header>
      <form
        className="flex gap-2 p-4"
        onSubmit={(e) => {
          e.preventDefault();
          void browse(path);
        }}
      >
        <button
          type="button"
          className="quiet-tool"
          aria-label="Parent directory"
          disabled={busy || !result?.parent}
          onClick={() => result?.parent && void browse(result.parent)}
        >
          <ArrowUp size={16} />
        </button>
        <input
          aria-label="Browse directory"
          className="min-w-0 flex-1 rounded border border-border bg-card px-2 py-2 font-mono text-xs"
          value={path}
          onChange={(e) => setPath(e.target.value)}
        />
        <button
          className="rounded border border-border px-3 text-xs"
          disabled={busy}
        >
          Go
        </button>
      </form>
      {error && (
        <p role="alert" className="px-4 pb-3 text-xs text-destructive">
          {error} Enter a parent directory or another path above.
        </p>
      )}
      <div className="min-h-24 overflow-y-auto px-4" aria-busy={busy}>
        {busy ? (
          <p className="p-4 text-sm text-muted-foreground">Loading paths…</p>
        ) : (
          !error &&
          result?.entries
            .filter((e) => !directory || e.is_directory)
            .map((entry) => (
              <button
                key={entry.path}
                type="button"
                className="flex w-full items-center gap-3 rounded px-2 py-2 text-left text-sm hover:bg-accent"
                onClick={() =>
                  entry.is_directory
                    ? void browse(entry.path)
                    : onSelect(entry.path)
                }
              >
                {entry.is_directory ? (
                  <Folder size={16} />
                ) : (
                  <FileText size={16} />
                )}
                <span className="min-w-0 break-all">{entry.name}</span>
              </button>
            ))
        )}
        {!busy &&
          !error &&
          result &&
          !result.entries.filter((e) => !directory || e.is_directory)
            .length && (
            <p className="py-4 text-sm text-muted-foreground">
              No {directory ? "subdirectories" : "files"} found.
            </p>
          )}
        {result?.truncated && (
          <p className="py-2 text-xs text-yellow-500">
            Only the first entries are shown. Enter a more specific path above.
          </p>
        )}
      </div>
      <footer className="flex justify-end gap-2 border-t border-border p-4">
        <button
          type="button"
          className="rounded px-3 py-2 text-xs"
          onClick={onClose}
        >
          Cancel
        </button>
        {directory && (
          <button
            type="button"
            disabled={busy || !!error || !result}
            className="rounded bg-primary px-3 py-2 text-xs text-primary-foreground disabled:opacity-40"
            onClick={() => result && onSelect(result.path)}
          >
            Choose this directory
          </button>
        )}
      </footer>
    </Modal>
  );
}
export function PathInput({
  label,
  value,
  onChange,
  onSave,
  environment,
  directory = true,
  invalid,
  placeholder,
  disabled = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onSave: (value: string) => void;
  environment?: string;
  directory?: boolean;
  invalid?: boolean;
  placeholder?: string;
  disabled?: boolean;
}) {
  const [picker, setPicker] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  return (
    <div
      className="quiet-path"
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <div className="quiet-input">
        <input
          ref={input}
          aria-label={label}
          title={value}
          aria-invalid={invalid}
          disabled={disabled}
          value={value}
          placeholder={placeholder}
          onChange={(e) => onChange(e.target.value)}
          onBlur={(e) => {
            if (
              !e.currentTarget.parentElement?.parentElement?.contains(
                e.relatedTarget,
              )
            )
              onSave(e.target.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              e.currentTarget.blur();
            }
          }}
        />
        <button
          type="button"
          className="quiet-clear"
          aria-label={`Clear ${label}`}
          title="Clear path"
          onMouseDown={(e) => e.preventDefault()}
          onClick={(e) => {
            e.preventDefault();
            onChange("");
            input.current?.focus();
          }}
        >
          <X size={13} />
        </button>
      </div>
      {environment && (
        <button
          type="button"
          className="quiet-tool"
          aria-label={`Browse ${label}`}
          title="Choose path"
          disabled={disabled}
          onClick={(e) => {
            e.preventDefault();
            setPicker(true);
          }}
        >
          <FolderSearch size={15} />
        </button>
      )}
      {picker && environment && (
        <PathPicker
          environment={environment}
          initialPath={value || "~"}
          directory={directory}
          onClose={() => setPicker(false)}
          onSelect={(path) => {
            setPicker(false);
            onChange(path);
            onSave(path);
          }}
        />
      )}
    </div>
  );
}
