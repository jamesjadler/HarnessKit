import {
  ArrowDown,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  Redo2,
  Undo2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { changedSpan, lineCount, mergeBlocks } from "@/lib/merge-blocks";
import "./text-comparison.css";
export interface ComparisonSide {
  id: string;
  environment: string;
  provider: string;
  path: string;
  revision: string;
  revisionName?: string;
  readOnly?: boolean;
  content: string;
  capturedAt?: string;
}
interface Props {
  left: ComparisonSide;
  right: ComparisonSide;
  third?: ComparisonSide;
  onSave: (side: ComparisonSide, content: string) => Promise<ComparisonSide>;
  onStopSync?: () => Promise<void>;
  onDirtyChange?: (dirty: boolean) => void;
  onBusyChange?: (busy: boolean) => void;
}
interface Draft {
  texts: string[];
  resolved: boolean[];
  ignored: string[][];
}
const revisionLabel = (r: string) =>
  /^[a-f0-9]{64}$/.test(r) ? `snapshot ${r.slice(0, 8)}` : r;
const copy = (d: Draft): Draft => ({
  texts: [...d.texts],
  resolved: [...d.resolved],
  ignored: d.ignored.map((x) => [...x]),
});
/** Sources are frozen until the parent explicitly reloads. Returned save revisions
 * are retained separately so a partially successful apply can be retried safely. */
export function TextComparison({
  left,
  right,
  third,
  onSave,
  onStopSync,
  onDirtyChange,
  onBusyChange,
}: Props) {
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const [sources] = useState(() => [left, right, ...(third ? [third] : [])]);
  const [saved, setSaved] = useState(sources);
  const blocks = useMemo(
    () => mergeBlocks(sources.map((s) => s.content)),
    [sources],
  );
  const initial = useMemo<Draft>(
    () => ({
      texts: blocks.map((b) => b.texts[0]),
      resolved: blocks.map((b) => !b.changed),
      ignored: blocks.map(() => []),
    }),
    [blocks],
  );
  const [draft, setDraft] = useState(initial);
  const [selected, setSelected] = useState<[number, number]>([0, 1]);
  const [current, setCurrent] = useState(() =>
    Math.max(
      0,
      blocks.findIndex((b) => b.changed),
    ),
  );
  const [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [message, setMessage] = useState("");
  const [highlight, setHighlight] = useState(true),
    [linked, setLinked] = useState(true);
  const [past, setPast] = useState<Draft[]>([]),
    [future, setFuture] = useState<Draft[]>([]);
  const [dirty, setDirty] = useState(false);
  const rows = useRef<(HTMLDivElement | null)[]>([]);
  const fields = useRef<(HTMLTextAreaElement | null)[]>([]);
  const scrollers = useRef<(HTMLElement | null)[]>([]);
  const changed = blocks.flatMap((b, i) => (b.changed ? [i] : []));
  const pending = draft.resolved.filter((x) => !x).length;
  const content = draft.texts.join("");
  const writable = saved.filter(
    (s, i, all) => !s.readOnly && all.findIndex((v) => v.id === s.id) === i,
  );
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => {
    if (!dirty) return;
    const warn = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [dirty]);
  const update = (fn: (next: Draft) => void) => {
    setPast((v) => [...v.slice(-99), copy(draft)]);
    setFuture([]);
    const next = copy(draft);
    fn(next);
    setDraft(next);
    setDirty(true);
    setMessage("");
  };
  const history = (redo: boolean) => {
    const from = redo ? future : past;
    if (!from.length) return;
    const previous = from[from.length - 1];
    if (redo) {
      setPast((v) => [...v, copy(draft)]);
      setFuture((v) => v.slice(0, -1));
    } else {
      setFuture((v) => [...v, copy(draft)]);
      setPast((v) => v.slice(0, -1));
    }
    setDraft(copy(previous));
    setDirty(true);
    setMessage("");
  };
  const go = (direction: number) => {
    const index = changed.indexOf(current);
    const next =
      changed[
        (Math.max(0, index) + direction + changed.length) % changed.length
      ];
    if (next === undefined) return;
    setCurrent(next);
    rows.current[next]?.scrollIntoView?.({
      block: "nearest",
      behavior: "smooth",
    });
    fields.current[next]?.focus({ preventScroll: true });
  };
  const apply = async () => {
    setBusy(true);
    onBusyChange?.(true);
    setError("");
    setMessage("");
    let count = 0;
    try {
      for (const side of writable) {
        if (!mounted.current) return;
        if (side.content === content) continue;
        const result = await onSave(side, content);
        setSaved((old) => old.map((s) => (s.id === side.id ? result : s)));
        count++;
      }
      setDirty(false);
      setMessage(
        `Result applied to ${writable.length} writable ${writable.length === 1 ? "file" : "files"}.`,
      );
    } catch (e) {
      setDirty(true);
      setError(
        `${count ? `${count} file(s) saved. ` : ""}${e instanceof Error ? e.message : String(e)}. Draft retained; reload before retrying an externally changed file.`,
      );
    } finally {
      setBusy(false);
      onBusyChange?.(false);
    }
  };
  const syncScroll = (element: HTMLElement) => {
    if (!linked) return;
    for (const pane of scrollers.current)
      if (
        pane &&
        pane !== element &&
        Math.abs(pane.scrollLeft - element.scrollLeft) > 1
      )
        pane.scrollLeft = element.scrollLeft;
  };
  const sourceHeader = (slot: 0 | 1) => {
    const side = sources[selected[slot]];
    return (
      <header className="merge-source-head">
        <select
          aria-label={slot ? "Right source" : "Left source"}
          value={selected[slot]}
          disabled={busy}
          onChange={(e) =>
            setSelected((old) =>
              slot
                ? [old[0], Number(e.target.value)]
                : [Number(e.target.value), old[1]],
            )
          }
        >
          {sources.map((s, i) => (
            <option
              key={`${s.id}-${i}`}
              value={i}
              disabled={i === selected[1 - slot]}
            >
              {s.environment} · {s.provider}
            </option>
          ))}
        </select>
        <span>
          {side.revisionName ?? revisionLabel(side.revision)}
          {side.readOnly ? " · Historical / read-only" : " · Read-only source"}
        </span>
        <code title={side.path}>{side.path}</code>
      </header>
    );
  };
  const sourceBlock = (source: number, index: number, height: number) => {
    const text = blocks[index].texts[source];
    const start =
      1 +
      blocks
        .slice(0, index)
        .reduce((n, b) => n + lineCount(b.texts[source]), 0);
    const [a, b] =
      highlight && blocks[index].changed
        ? changedSpan(text, draft.texts[index])
        : [0, 0];
    const lines = text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
    let offset = 0;
    return (
      <div
        className={`merge-source-code ${blocks[index].changed ? "merge-changed" : ""} ${!text ? "merge-empty" : ""}`}
        style={{ minHeight: height }}
        ref={(el) => {
          scrollers.current[index * 3 + (source === selected[0] ? 0 : 2)] = el;
        }}
        onScroll={(e) => syncScroll(e.currentTarget)}
      >
        {lines.map((line, i) => {
          const begin = offset;
          offset += line.length;
          const lo = Math.max(0, a - begin),
            hi = Math.min(line.length, b - begin);
          return (
            <div className="merge-line" key={i}>
              <span className="merge-number">{start + i}</span>
              <code>
                {hi > lo ? (
                  <>
                    {line.slice(0, lo)}
                    <mark>{line.slice(lo, hi)}</mark>
                    {line.slice(hi)}
                  </>
                ) : (
                  line
                )}
              </code>
            </div>
          );
        })}
      </div>
    );
  };
  const controls = (slot: 0 | 1, i: number) => {
    const s = sources[selected[slot]],
      done = draft.resolved[i] || draft.ignored[i].includes(s.id);
    const accept = (
      <button
        title={`Accept ${s.environment} change`}
        aria-label={`Use ${s.environment} change ${i + 1}`}
        disabled={busy}
        onClick={() => {
          setCurrent(i);
          update((d) => {
            d.texts[i] = blocks[i].texts[selected[slot]];
            d.resolved[i] = true;
            d.ignored[i] = [];
          });
        }}
      >
        {slot ? <ChevronLeft size={15} /> : <ChevronRight size={15} />}
      </button>
    );
    const ignore = (
      <button
        title={`Ignore ${s.environment} change; keep result unchanged`}
        aria-label={`Ignore ${s.environment} change ${i + 1}`}
        aria-pressed={draft.ignored[i].includes(s.id)}
        disabled={busy}
        onClick={() => {
          setCurrent(i);
          update((d) => {
            d.ignored[i] = [...new Set([...d.ignored[i], s.id])];
            if (sources.every((v) => d.ignored[i].includes(v.id)))
              d.resolved[i] = true;
          });
        }}
      >
        <X size={13} />
      </button>
    );
    return (
      <div className={`merge-gutter ${done ? "merge-handled" : ""}`}>
        {blocks[i].changed && (
          <>
            <svg
              className="merge-connector"
              viewBox="0 0 44 100"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <path d="M0 0 C22 0 22 4 44 4 L44 96 C22 96 22 100 0 100 Z" />
            </svg>
            <div className="merge-actions">
              {slot ? (
                <>
                  {accept}
                  {ignore}
                </>
              ) : (
                <>
                  {ignore}
                  {accept}
                </>
              )}
            </div>
          </>
        )}
      </div>
    );
  };
  if (new Set(sources.map((s) => s.id)).size < 2) {
    return (
      <p
        role="status"
        className="rounded-lg border border-border bg-card p-4 text-sm text-muted-foreground"
      >
        No other matching copy selected.
      </p>
    );
  }
  if (!dirty && saved.every((s) => s.content === saved[0].content)) {
    return (
      <p
        role="status"
        className="rounded-lg border border-border bg-card p-4 text-sm text-[var(--trust-safe)]"
      >
        Contents match across the selected copies. No diff needed.
      </p>
    );
  }
  return (
    <section aria-label="Text comparison" className="merge-editor">
      <div className="merge-title">
        <b>Merge {left.path.split(/[\\/]/).pop()}</b>
        <span>
          {pending ? `${pending} unresolved changes` : "All changes resolved"}
        </span>
      </div>
      <div className="merge-toolbar">
        <button disabled={busy || !changed.length} onClick={() => go(-1)}>
          <ArrowUp size={14} /> Previous change
        </button>
        <button disabled={busy || !changed.length} onClick={() => go(1)}>
          <ArrowDown size={14} /> Next change
        </button>
        <label>
          <input
            type="checkbox"
            checked={highlight}
            onChange={(e) => setHighlight(e.target.checked)}
          />{" "}
          Highlight words
        </label>
        <label>
          <input
            type="checkbox"
            checked={linked}
            onChange={(e) => setLinked(e.target.checked)}
          />{" "}
          Linked scrolling
        </label>
        <button
          disabled={busy || draft.resolved[current]}
          onClick={() =>
            update((d) => {
              d.resolved[current] = true;
            })
          }
        >
          Mark block resolved
        </button>
        <button
          aria-label="Undo"
          title="Undo"
          disabled={busy || !past.length}
          onClick={() => history(false)}
        >
          <Undo2 size={15} />
        </button>
        <button
          aria-label="Redo"
          title="Redo"
          disabled={busy || !future.length}
          onClick={() => history(true)}
        >
          <Redo2 size={15} />
        </button>
      </div>
      {error && (
        <p role="alert" className="merge-error">
          {error}
        </p>
      )}
      {message && (
        <p role="status" className="merge-success">
          {message}
        </p>
      )}
      <div className="merge-viewport">
        <div className="merge-wide">
          <div className="merge-heads">
            {sourceHeader(0)}
            <header className="merge-result-head">
              <b>Result · editable</b>
              <span>{dirty ? "Unsaved changes" : "Draft"}</span>
              <span>
                Apply to:{" "}
                {writable.map((s) => s.environment).join(", ") ||
                  "No writable targets"}
              </span>
            </header>
            {sourceHeader(1)}
          </div>
          {blocks.map((block, i) => {
            const text = draft.texts[i];
            const height =
              Math.max(
                1,
                ...block.texts.map((t) =>
                  Math.max(
                    1,
                    t.split("\n").length - (t.endsWith("\n") ? 1 : 0),
                  ),
                ),
                text.split("\n").length,
              ) * 22;
            const start =
              1 + draft.texts.slice(0, i).reduce((n, t) => n + lineCount(t), 0);
            return (
              <div
                key={i}
                ref={(el) => {
                  rows.current[i] = el;
                }}
                className={`merge-row ${current === i ? "merge-current" : ""}`}
              >
                {sourceBlock(selected[0], i, height)}
                {controls(0, i)}
                <div
                  className={`merge-result-code ${block.changed ? (draft.resolved[i] ? "merge-resolved" : "merge-changed") : ""}`}
                >
                  <div className="merge-result-numbers" aria-hidden="true">
                    {text.split("\n").map((_, n) => (
                      <div key={n}>
                        {text.endsWith("\n") && n === lineCount(text)
                          ? ""
                          : start + n}
                      </div>
                    ))}
                  </div>
                  <textarea
                    ref={(el) => {
                      fields.current[i] = el;
                      scrollers.current[i * 3 + 1] = el;
                    }}
                    aria-label={`Result block ${i + 1}`}
                    value={text}
                    disabled={busy}
                    spellCheck={false}
                    wrap="off"
                    style={{ height }}
                    onFocus={() => setCurrent(i)}
                    onScroll={(e) => syncScroll(e.currentTarget)}
                    onChange={(e) =>
                      update((d) => {
                        // Textareas normalize CRLF. Restore the block's existing convention.
                        const previous = d.texts[i] || sources[0].content;
                        d.texts[i] =
                          previous.includes("\r\n") &&
                          !/(^|[^\r])\n/.test(previous)
                            ? e.target.value.replace(/\r?\n/g, "\r\n")
                            : e.target.value;
                        d.resolved[i] = !block.changed;
                        d.ignored[i] = [];
                      })
                    }
                    onKeyDown={(e) => {
                      if (
                        (e.ctrlKey || e.metaKey) &&
                        e.key.toLowerCase() === "z"
                      ) {
                        e.preventDefault();
                        history(e.shiftKey);
                      }
                    }}
                  />
                </div>
                {controls(1, i)}
                {sourceBlock(selected[1], i, height)}
              </div>
            );
          })}
        </div>
      </div>
      <div className="merge-footer">
        {sources.map((s, index) => (
          <button
            key={`${s.id}-${index}`}
            disabled={busy}
            onClick={() =>
              update((d) => {
                d.texts = blocks.map((b) => b.texts[index]);
                d.resolved = blocks.map(() => true);
                d.ignored = blocks.map(() => []);
              })
            }
          >
            Keep {s.environment} · {s.revisionName ?? revisionLabel(s.revision)}
          </button>
        ))}
        {onStopSync && (
          <button
            disabled={busy}
            onClick={async () => {
              if (
                dirty &&
                !window.confirm("Discard unsaved merge edits and stop syncing?")
              )
                return;
              setBusy(true);
              try {
                await onStopSync();
              } catch (e) {
                setError(String(e));
              } finally {
                setBusy(false);
              }
            }}
          >
            Stop syncing this skill
          </button>
        )}
        <button
          className="merge-apply"
          disabled={busy || pending > 0 || !writable.length}
          onClick={apply}
        >
          {busy ? "Applying…" : "Save & apply result"}
        </button>
        <p>
          Arrows accept a source change; × ignores that side without deleting
          text. Keep selects a whole version. Save explicitly writes the result
          to the writable files above, checking each revision. Historical
          sources remain unchanged.
        </p>
      </div>
    </section>
  );
}
