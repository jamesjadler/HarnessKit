import { useEffect, useRef, useState } from "react";
import { useEnvironmentStore } from "@/stores/environment-store";

// Drafts survive a failed save and closing/reopening Settings.
const drafts = new Map<string, string>();
export function ProviderPathInput({
  environment,
  provider,
  path,
}: {
  environment: string;
  provider: string;
  path: string;
}) {
  const key = `${environment}:${provider}`;
  const [value, setValue] = useState(drafts.get(key) ?? path);
  const [error, setError] = useState<string | null>(null);
  const state = useRef({ value, expected: path });
  state.current.value = value;
  useEffect(() => {
    if (!drafts.has(key)) {
      state.current.expected = path;
      setValue(path);
    }
  }, [key, path]);
  const queue = useRef(Promise.resolve());
  const save = useRef<() => void>(() => {});
  save.current = () => {
    queue.current = queue.current.then(async () => {
      const root = state.current.value.trim();
      if (!drafts.has(key) || root === state.current.expected) return;
      const expected = state.current.expected;
      try {
        setError(null);
        await useEnvironmentStore.getState().command({
          action: "provider_path",
          environment,
          provider,
          expected,
          root,
        });
        state.current.expected = root;
        if (drafts.get(key)?.trim() === root) drafts.delete(key);
      } catch (e) {
        setError(String(e));
      }
    });
  };
  useEffect(() => {
    const flush = () => save.current();
    window.addEventListener("harnesskit-save-provider-paths", flush);
    return () => {
      window.removeEventListener("harnesskit-save-provider-paths", flush);
      flush();
    };
  }, []);
  return (
    <div className="min-w-0 flex-1">
      <input
        aria-invalid={!!error}
        aria-label={`${provider} skills directory`}
        className="w-full min-w-0 rounded border border-border bg-background p-2 font-mono text-xs"
        value={value}
        onChange={(e) => {
          drafts.set(key, e.target.value);
          setValue(e.target.value);
        }}
      />
      {error && (
        <p role="alert" className="mt-1 text-xs text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
