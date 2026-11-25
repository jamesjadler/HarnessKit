import {
  Check,
  ChevronRight,
  Copy,
  Laptop,
  Monitor,
  Plus,
  RefreshCw,
  Terminal,
  TriangleAlert,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { DiffIcon } from "@/components/shared/diff-icon";
import {
  TextComparison,
  type ComparisonSide,
} from "@/components/shared/text-comparison";
import { VariantNameDialog } from "@/components/shared/variant-name-dialog";
import { Modal } from "@/components/ui/modal";
import { copyPathToClipboard } from "@/lib/copy-path";
import {
  latestRevision,
  managedStatus,
  type ManagedFile,
  type ManagedFileCopy,
  type ManagedFileRequest,
} from "@/lib/managed-files";
import { useManagedFileStore } from "@/stores/managed-file-store";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import "./managed-files.css";
const drafts = new Map<string, { text: string; expected: string }>();
const side = (file: ManagedFile, c: ManagedFileCopy): ComparisonSide => ({
  id: c.id,
  environment: `${c.machine} / ${c.environment}`,
  provider: file.agent,
  path: c.path,
  revision: c.fingerprint ?? "",
  content: c.content ?? "",
  readOnly: !!c.error || !c.enabled,
});
function CopyIcon({
  copy,
  file,
}: {
  copy: ManagedFileCopy;
  file: ManagedFile;
}) {
  const good =
    !copy.error &&
    !copy.pending &&
    copy.fingerprint === latestRevision(file)?.fingerprint;
  const Icon = /windows/i.test(copy.environment)
    ? Monitor
    : /mac/i.test(copy.environment)
      ? Laptop
      : Terminal;
  const label = `${copy.machine} / ${copy.environment} · ${copy.error ?? (!copy.enabled ? "Sync off" : good ? "Synced" : "Not-synced")}`;
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      className={`managed-env ${!copy.enabled ? "muted" : good ? "synced" : copy.error && /permission|denied/i.test(copy.error) ? "denied" : "attention"}`}
    >
      <Icon size={15} />
    </span>
  );
}
export function ManagedFileRow({
  file,
  variantExtras,
}: {
  file: ManagedFile;
  variantExtras?: ReactNode;
}) {
  const rev = latestRevision(file),
    status = managedStatus(file),
    command = useManagedFileStore((s) => s.command),
    busy = useManagedFileStore((s) => s.busy);
  const focus = useAgentConfigStore((s) => s.pendingFocusFile);
  const [open, setOpen] = useState(false),
    [text, setText] = useState(
      () => drafts.get(file.id)?.text ?? rev?.content ?? "",
    ),
    [expected, setExpected] = useState(
      () => drafts.get(file.id)?.expected ?? rev?.fingerprint ?? "",
    ),
    [error, setError] = useState(""),
    [notice, setNotice] = useState(""),
    [variant, setVariant] = useState(false);
  const [comparison, setComparison] = useState<ComparisonSide[] | null>(null),
    [compareDirty, setCompareDirty] = useState(false),
    [compareBusy, setCompareBusy] = useState(false);
  const [comparisonExpected, setComparisonExpected] = useState("");
  const dirty = drafts.has(file.id);
  useEffect(() => {
    if (!drafts.has(file.id)) {
      setText(rev?.content ?? "");
      setExpected(rev?.fingerprint ?? "");
    }
  }, [file.id, rev?.fingerprint, rev?.content]);
  useEffect(() => {
    if (focus && file.copies.some((c) => c.path === focus)) {
      setOpen(true);
      document
        .getElementById(`managed-${file.id}`)
        ?.scrollIntoView?.({ block: "nearest" });
    }
  }, [focus, file]);
  useEffect(() => {
    if (!dirty && !compareDirty) return;
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [dirty, compareDirty]);
  const run = async (
    req: ManagedFileRequest,
    discard = false,
    confirmed = false,
  ) => {
    if (
      discard &&
      dirty &&
      !confirmed &&
      !window.confirm("Discard unsaved text changes?")
    )
      return;
    setError("");
    setNotice("");
    try {
      const result = await command({ file_id: file.id, ...req });
      if (discard || req.action === "save") {
        drafts.delete(file.id);
        const updated = result.files.find((f) => f.id === file.id),
          latest = updated && latestRevision(updated);
        setText(latest?.content ?? text);
        setExpected(latest?.fingerprint ?? expected);
      }
      setNotice(
        req.action === "retry"
          ? "Sync retried."
          : req.action === "variant"
            ? "Independent variant created."
            : "Saved. Sync status is shown for each environment.",
      );
    } catch (e) {
      setError(String(e));
    }
  };
  const compare = () => {
    setComparisonExpected(rev?.fingerprint ?? "");
    const copies = file.copies.filter((c) => c.content !== null && !c.error);
    if (copies.length < 2) return;
    const first = copies[0],
      second =
        copies.find((c) => c.fingerprint !== first.fingerprint) ?? copies[1];
    setComparison(
      [
        first,
        second,
        ...copies
          .filter((c) => c.id !== first.id && c.id !== second.id)
          .slice(0, 1),
      ].map((c) => side(file, c)),
    );
  };
  const closeCompare = () => {
    if (
      !compareBusy &&
      (!compareDirty || window.confirm("Discard unsaved comparison edits?"))
    ) {
      setComparison(null);
      setCompareDirty(false);
    }
  };
  return (
    <div id={`managed-${file.id}`} className="managed-file">
      <div className="managed-row">
        <button
          className="managed-name"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
        >
          <ChevronRight size={14} className={open ? "expanded" : ""} />
          <b>{file.file_name}</b>
          {file.variant_name && (
            <span className="managed-badge">{file.variant_name}</span>
          )}
          <span className="managed-badge">
            {file.scope === "global" ? "Global" : "Project"}
          </span>
        </button>
        <div className="managed-envs">
          {file.copies.map((c) => (
            <CopyIcon key={c.id} copy={c} file={file} />
          ))}
        </div>
        <div
          className={`managed-status ${status === "Synced" ? "synced" : status === "Not assigned" || status === "Sync off" ? "muted" : "attention"}`}
        >
          {status === "Synced" && <Check size={13} />}
          <span>{status}</span>
          {(status === "Not-synced" || status === "Partial") &&
            file.copies.filter((c) => c.content !== null && !c.error).length >=
              2 && (
              <button
                className="managed-icon"
                title="Compare environment copies"
                aria-label={`Compare ${file.file_name} copies`}
                onClick={compare}
              >
                <DiffIcon size={14} />
              </button>
            )}
        </div>
      </div>
      {open && (
        <div className="managed-expanded">
          <details>
            <summary>
              Environment paths{" "}
              <span className="managed-badge">{file.copies.length}</span>
            </summary>
            <div>
              {file.copies.length === 0 && (
                <p className="managed-note">
                  This named duplicate is stored independently and has no
                  assigned environment copies.
                </p>
              )}
              {file.copies.map((c) => (
                <div key={c.id} className="managed-path">
                  <div>
                    <b>{c.environment}</b>
                    <small>{c.machine}</small>
                  </div>
                  <code>{c.path}</code>
                  <span className={c.error ? "attention" : "muted"}>
                    {c.error ??
                      (!c.enabled
                        ? "Sync off"
                        : c.pending
                          ? "Background change"
                          : c.fingerprint === rev?.fingerprint
                            ? `r${rev?.number}`
                            : "Not-synced")}
                  </span>
                  <button
                    className="managed-icon"
                    title={`Copy ${c.environment} path`}
                    aria-label={`Copy ${c.environment} path`}
                    onClick={() =>
                      copyPathToClipboard(c.path).catch((e) =>
                        setError(String(e)),
                      )
                    }
                  >
                    <Copy size={14} />
                  </button>
                </div>
              ))}
            </div>
          </details>
          <details>
            <summary>
              Version history{" "}
              <span className="managed-badge">{rev?.number ?? 0}</span>
              <span className="managed-summary-note">
                Current r{rev?.number ?? 0}
              </span>
            </summary>
            {[...file.revisions]
              .sort((a, b) => b.number - a.number)
              .map((r) => (
                <div className="managed-history" key={r.number}>
                  <div>
                    <b>
                      r{r.number}
                      {r.number === rev?.number ? " · Current" : ""}
                    </b>
                    <small>
                      {r.reason} · {new Date(r.created_at).toLocaleString()}
                    </small>
                  </div>
                  <button
                    className="managed-icon"
                    title={`Compare r${r.number} with current`}
                    aria-label={`Compare revision ${r.number}`}
                    onClick={() =>
                      setComparison([
                        {
                          id: `revision-${r.number}`,
                          environment: `r${r.number}`,
                          provider: file.agent,
                          path: file.file_name,
                          revision: r.fingerprint,
                          content: r.content,
                          readOnly: true,
                        },
                        {
                          id: "current-history",
                          environment: `Current r${rev?.number}`,
                          provider: file.agent,
                          path: file.file_name,
                          revision: rev?.fingerprint ?? "",
                          content: rev?.content ?? "",
                          readOnly: true,
                        },
                      ])
                    }
                  >
                    <DiffIcon size={14} />
                  </button>
                  {r.number !== rev?.number && (
                    <button
                      className="managed-link"
                      disabled={busy}
                      onClick={() => {
                        if (
                          window.confirm(
                            `Are you sure you want to restore r${r.number}? This creates a new version and attempts to sync enabled copies.${dirty ? " Unsaved edits will be discarded." : ""}`,
                          )
                        )
                          void run(
                            {
                              action: "restore",
                              revision: r.number,
                              expected: rev?.fingerprint,
                            },
                            true,
                            true,
                          );
                      }}
                    >
                      Restore
                    </button>
                  )}
                </div>
              ))}
          </details>
          <details>
            <summary>
              Variants
              <button
                className="managed-create"
                disabled={busy || !rev}
                onClick={(e) => {
                  e.preventDefault();
                  setVariant(true);
                }}
              >
                <Plus size={12} /> Create variant
              </button>
            </summary>
            <VariantFamily file={file} />
            {variantExtras}
          </details>
          {file.copies
            .filter((c) => c.pending && c.content !== null)
            .map((c) => (
              <div className="managed-pending" key={c.id}>
                <b>
                  <TriangleAlert size={14} /> Background change ·{" "}
                  {c.environment}
                </b>
                <p>
                  Accepting records a new version and attempts to sync the
                  enabled copies.
                </p>
                <button
                  className="managed-icon"
                  aria-label={`Compare background change from ${c.environment}`}
                  title="Compare background change"
                  onClick={() =>
                    setComparison([
                      {
                        id: "accepted",
                        environment: `Accepted r${rev?.number}`,
                        provider: file.agent,
                        path: file.file_name,
                        revision: rev?.fingerprint ?? "",
                        content: rev?.content ?? "",
                        readOnly: true,
                      },
                      { ...side(file, c), readOnly: true },
                    ])
                  }
                >
                  <DiffIcon size={14} />
                </button>
                <button
                  disabled={busy}
                  className="managed-link"
                  onClick={() =>
                    void run(
                      {
                        action: "accept",
                        copy_id: c.id,
                        expected: c.fingerprint ?? "",
                      },
                      true,
                    )
                  }
                >
                  Accept & sync
                </button>
              </div>
            ))}
          <div className="managed-content">
            <div className="managed-content-head">
              <b>{file.file_name}</b>
              <span className="managed-badge">r{rev?.number ?? 0}</span>
              <span className="muted">
                {status === "Synced"
                  ? "Same content in all enabled environments"
                  : status}
              </span>
            </div>
            {rev ? (
              <textarea
                aria-label={`${file.file_name} contents`}
                spellCheck={false}
                value={text}
                disabled={busy}
                onChange={(e) => {
                  const value =
                    rev.content.includes("\r\n") &&
                    !/(^|[^\r])\n/.test(rev.content)
                      ? e.target.value.replace(/\r?\n/g, "\r\n")
                      : e.target.value;
                  setText(value);
                  if (value === rev.content) drafts.delete(file.id);
                  else drafts.set(file.id, { text: value, expected });
                }}
              />
            ) : (
              <p className="managed-note">
                No readable text version is available. Check the environment
                paths above.
              </p>
            )}
            <div className="managed-savebar">
              <span className="muted">
                {dirty ? "Unsaved changes" : "Saved"}
              </span>
              {status !== "Synced" && file.copies.length > 0 && (
                <button
                  className="managed-icon"
                  disabled={busy}
                  title="Retry sync"
                  aria-label="Retry sync"
                  onClick={() => void run({ action: "retry" })}
                >
                  <RefreshCw size={14} />
                </button>
              )}
              <button
                className="managed-link"
                disabled={!dirty || busy}
                onClick={() => {
                  drafts.delete(file.id);
                  setText(rev?.content ?? "");
                  setExpected(rev?.fingerprint ?? "");
                }}
              >
                Discard
              </button>
              <button
                className="managed-save"
                disabled={!dirty || busy || !rev}
                onClick={() =>
                  void run({ action: "save", content: text, expected })
                }
              >
                {file.copies.length ? "Save & sync" : "Save version"}
              </button>
            </div>
          </div>
          {error && (
            <p role="alert" className="text-xs text-destructive">
              {error}
            </p>
          )}
          {notice && (
            <p role="status" className="managed-note">
              {notice}
            </p>
          )}
        </div>
      )}
      {variant && (
        <VariantNameDialog
          source={`${file.file_name} r${rev?.number}`}
          onClose={() => setVariant(false)}
          onCreate={async (name) => {
            await command({
              action: "variant",
              file_id: file.id,
              expected: rev?.fingerprint,
              name,
            });
          }}
        />
      )}
      {comparison && (
        <Modal
          ariaLabel="Compare file versions"
          onClose={closeCompare}
          busy={compareBusy}
          containerClassName="flex max-h-[90vh] w-[min(1500px,96vw)] flex-col overflow-auto rounded-xl border border-border bg-background p-3"
        >
          <button
            className="self-end text-xs p-2"
            disabled={compareBusy}
            onClick={closeCompare}
          >
            Close comparison
          </button>
          <TextComparison
            key={
              comparisonExpected + comparison.map((c) => c.revision).join(":")
            }
            left={comparison[0]}
            right={comparison[1]}
            third={comparison[2]}
            onDirtyChange={setCompareDirty}
            onBusyChange={setCompareBusy}
            onSave={async () => {
              throw new Error("Use Save & sync to record a managed version");
            }}
            onApplyResult={
              comparison.every((c) => c.readOnly)
                ? undefined
                : async (content) => {
                    const result = await command({
                      action: "save",
                      file_id: file.id,
                      content,
                      expected: comparisonExpected,
                      observed: Object.fromEntries(
                        comparison
                          .filter((c) => !c.readOnly)
                          .map((c) => [c.id, c.revision]),
                      ),
                    });
                    const updated = result.files.find((f) => f.id === file.id);
                    if (updated) {
                      setComparison(
                        comparison.map((old) => {
                          const copy = updated.copies.find(
                            (c) => c.id === old.id,
                          );
                          return copy ? side(updated, copy) : old;
                        }),
                      );
                      setComparisonExpected(
                        latestRevision(updated)?.fingerprint ??
                          comparisonExpected,
                      );
                    }
                  }
            }
          />
        </Modal>
      )}
    </div>
  );
}
function VariantFamily({ file }: { file: ManagedFile }) {
  const files = useManagedFileStore((s) => s.files);
  const id = file.original_id ?? file.id;
  return (
    <div className="managed-note">
      {files
        .filter((f) => f.id === id || f.original_id === id)
        .map((f) => (
          <div key={f.id} className="managed-family">
            <span>
              {f.variant_name ?? "Default"} · r{latestRevision(f)?.number ?? 0}
            </span>
            <small>{f.id}</small>
          </div>
        ))}
    </div>
  );
}
export function ManagedFileList({ files }: { files: ManagedFile[] }) {
  const groups = new Map<string, ManagedFile[]>();
  for (const file of files) {
    const label =
      file.scope === "global"
        ? file.category
        : `Project · ${file.scope_path ?? file.scope} · ${file.category}`;
    groups.set(label, [...(groups.get(label) ?? []), file]);
  }
  return (
    <div className="managed-files">
      {[...groups].map(([name, items]) => (
        <section key={name} className="mb-5">
          <h3 className="managed-section-title">
            {name} <span className="managed-badge">{items.length}</span>
          </h3>
          {items.map((f) => (
            <ManagedFileRow key={f.id} file={f} />
          ))}
        </section>
      ))}
    </div>
  );
}
