import { useEffect, useMemo, useState } from "react";
import { transport } from "@/lib/transport";
import { useEnvironmentStore } from "@/stores/environment-store";
import { type ComparisonSide, TextComparison } from "./text-comparison";

interface Snapshot {
  path: string;
  content: string;
  revision: string;
  captured_at: string;
}
export interface FileChoice {
  path: string;
  provider: string;
  environment?: string;
  environmentId?: string;
  providerId?: string;
  /** Logical scope identity, never just the project display name. */
  scope?: string;
  category?: string;
}
const choiceId = (choice: FileChoice) =>
  `${choice.environmentId ?? "local"}:${choice.providerId ?? choice.provider}:${choice.path}`;
const fileName = (path: string) => path.replace(/\\/g, "/").split("/").pop();
export function isComparableFile(source: FileChoice, candidate: FileChoice) {
  return (
    source.provider === candidate.provider &&
    fileName(source.path) === fileName(candidate.path) &&
    (source.scope ?? "global") === (candidate.scope ?? "global") &&
    (!source.category ||
      !candidate.category ||
      source.category === candidate.category)
  );
}
export function FileComparison({
  source,
  choices,
  onClose,
}: {
  source: FileChoice;
  choices: FileChoice[];
  onClose: () => void;
}) {
  const environments = useEnvironmentStore(
    (s) => s.data?.registry.environments,
  );
  const remoteKey = JSON.stringify(
    environments
      ?.filter((e) => e.id !== "local")
      .map((e) => ({
        id: e.id,
        name: e.name,
        machine: e.machine,
        providers: e.providers
          .filter(
            (p) =>
              !p.last_error &&
              !p.disabled &&
              p.name === source.provider &&
              (!p.scope || p.scope === "global") &&
              /[\\/]skills[\\/]?$/.test(p.root),
          )
          .map((p) => ({
            id: p.id,
            name: p.name,
            root: p.root,
          })),
      })) ?? [],
  );
  const available = useMemo(() => {
    const file = source.path.split(/[\\/]/).pop() ?? "";
    const supported = [
      "config.toml",
      "AGENTS.md",
      "CLAUDE.md",
      "GEMINI.md",
      "settings.json",
      "config.json",
      "mcp.json",
    ].includes(file);
    const remote: FileChoice[] =
      supported && (source.scope ?? "global") === "global"
        ? JSON.parse(remoteKey).flatMap(
            (env: {
              id: string;
              name: string;
              machine: string;
              providers: { id: string; name: string; root: string }[];
            }) =>
              env.providers.map((p) => ({
                path: p.root.replace(/[\\/][^\\/]+[\\/]?$/, "") + "/" + file,
                provider: p.name,
                scope: "global",
                category: source.category,
                providerId: p.id,
                environment: `${env.machine} / ${env.name}`,
                environmentId: env.id,
              })),
          )
        : [];
    return [source, ...choices, ...remote].filter(
      (c, i, all) =>
        isComparableFile(source, c) &&
        all.findIndex((v) => choiceId(v) === choiceId(c)) === i,
    );
  }, [choices, source, remoteKey]);
  const [target, setTarget] = useState(
    choiceId(available.find((c) => choiceId(c) !== choiceId(source)) ?? source),
  );
  const [thirdTarget, setThirdTarget] = useState("");
  const [selection, setSelection] = useState<FileChoice[]>(() => [
    source,
    available.find((c) => choiceId(c) !== choiceId(source)) ?? source,
  ]);
  const [pair, setPair] = useState<ComparisonSide[] | null>(null),
    [error, setError] = useState("");
  const [dirty, setDirty] = useState(false);
  const [applying, setApplying] = useState(false);
  const discard = () =>
    !applying &&
    (!dirty || window.confirm("Discard unsaved comparison edits?"));
  const [generation, setGeneration] = useState(0);
  // biome-ignore lint/correctness/useExhaustiveDependencies: generation explicitly reloads the snapshots.
  useEffect(() => {
    let active = true;
    setPair(null);
    setError("");
    const read = async (choice: FileChoice): Promise<ComparisonSide> => {
      const snapshot = await transport<Snapshot>("read_comparison_file", {
        path: choice.path,
        environment: choice.environmentId ?? null,
        provider: choice.providerId ?? null,
      });
      return {
        id: choiceId(choice),
        environment: choice.environment ?? "Local",
        provider: choice.provider,
        path: snapshot.path,
        revision: snapshot.revision,
        content: snapshot.content,
        capturedAt: snapshot.captured_at,
      };
    };
    Promise.all(selection.map(read))
      .then((p) => {
        if (active) setPair(p);
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    return () => {
      active = false;
    };
  }, [selection, generation]);
  return (
    <div className="my-3 min-w-0 space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <label className="text-xs">
          Compare with{" "}
          <select
            disabled={applying}
            className="max-w-full rounded border border-border bg-background p-2"
            value={target}
            onChange={(e) => {
              if (discard()) {
                setDirty(false);
                setTarget(e.target.value);
                setSelection([
                  selection[0],
                  available.find((c) => choiceId(c) === e.target.value) ??
                    source,
                  ...selection.slice(2),
                ]);
              }
            }}
          >
            {available.map((c) => (
              <option
                key={choiceId(c)}
                value={choiceId(c)}
                disabled={choiceId(c) === thirdTarget}
              >
                {c.environment ?? "Local"} · {c.provider} · {c.path}
              </option>
            ))}
          </select>
        </label>
        <label className="text-xs">
          Third environment (optional){" "}
          <select
            disabled={applying}
            aria-label="Third environment"
            className="max-w-full rounded border border-border bg-background p-2"
            value={thirdTarget}
            onChange={(e) => {
              if (!discard()) return;
              const choice = available.find(
                (c) => choiceId(c) === e.target.value,
              );
              setDirty(false);
              setThirdTarget(e.target.value);
              setSelection((old) => [
                old[0],
                old[1],
                ...(choice ? [choice] : []),
              ]);
            }}
          >
            <option value="">None</option>
            {available
              .filter(
                (c) =>
                  !selection
                    .slice(0, 2)
                    .some((s) => choiceId(s) === choiceId(c)),
              )
              .map((c) => (
                <option key={choiceId(c)} value={choiceId(c)}>
                  {c.environment ?? "Local"} · {c.provider} · {c.path}
                </option>
              ))}
          </select>
        </label>
        <button
          disabled={applying}
          className="text-xs underline"
          onClick={() => {
            if (discard()) {
              setDirty(false);
              setSelection([
                source,
                available.find((c) => choiceId(c) === target) ?? selection[1],
                ...selection.slice(2),
              ]);
              setGeneration((v) => v + 1);
            }
          }}
        >
          Reload snapshots (discard edits)
        </button>
        <button
          disabled={applying}
          className="text-xs underline"
          onClick={() => {
            if (discard()) onClose();
          }}
        >
          Close comparison
        </button>
      </div>
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
      {pair ? (
        <TextComparison
          onDirtyChange={setDirty}
          onBusyChange={setApplying}
          key={`${target}-${thirdTarget}-${generation}`}
          left={pair[0]}
          right={pair[1]}
          third={pair[2]}
          onSave={async (side, content) => {
            const choice = selection.find((c) => choiceId(c) === side.id);
            const snapshot = await transport<Snapshot>("save_comparison_file", {
              path: side.path,
              expected: side.revision,
              content,
              environment: choice?.environmentId ?? null,
              provider: choice?.providerId ?? null,
            });
            return { ...side, ...snapshot, capturedAt: snapshot.captured_at };
          }}
        />
      ) : (
        !error && (
          <p className="text-xs text-muted-foreground">
            Reading complete files…
          </p>
        )
      )}
    </div>
  );
}
