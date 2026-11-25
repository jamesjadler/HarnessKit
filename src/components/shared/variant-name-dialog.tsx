import { useId, useState } from "react";
import { Modal } from "@/components/ui/modal";
export function VariantNameDialog({
  source,
  onCreate,
  onClose,
}: {
  source: string;
  onCreate: (name: string) => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState(""),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const id = useId();
  return (
    <Modal ariaLabel="Create variant" onClose={onClose} busy={busy}>
      <form
        className="space-y-4 p-5"
        onSubmit={async (e) => {
          e.preventDefault();
          if (!name.trim()) return;
          setBusy(true);
          setError("");
          try {
            await onCreate(name.trim());
            onClose();
          } catch (e) {
            setError(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        <h2 className="text-lg font-semibold">Create variant</h2>
        <p className="text-xs text-muted-foreground">
          Duplicate {source} with a new identity and its own version history.
          The original stays unchanged.
        </p>
        <label htmlFor={id} className="block text-sm">
          Variant name
        </label>
        <input
          id={id}
          autoFocus
          required
          maxLength={80}
          value={name}
          disabled={busy}
          onChange={(e) => setName(e.target.value)}
          className="w-full rounded border border-border bg-background p-2"
        />
        {error && (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        )}
        <div className="flex justify-end gap-3">
          <button
            type="button"
            disabled={busy}
            onClick={onClose}
            className="text-xs"
          >
            Cancel
          </button>
          <button
            disabled={busy || !name.trim()}
            className="rounded bg-primary px-3 py-2 text-xs text-primary-foreground"
          >
            {busy ? "Creating…" : "Create variant"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
