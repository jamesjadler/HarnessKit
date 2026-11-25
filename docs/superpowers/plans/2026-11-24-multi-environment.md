# Multi-environment HarnessKit implementation plan

> Execute inline using superpowers:executing-plans. User approved implementation on 2026-11-24 after static revision 5; this explicitly advances past the separate prototype stage.

**Goal:** Implement the approved multi-environment skill management and shared comparison experience.
**Architecture:** Coordinator-owned Rust services provide persisted environment, revision and reconciliation records to both HTTP and Tauri. React consumes those services, keeping upstream inventory, permissions, provider icons and scope behavior.
**Tech stack:** Rust, serde, filesystem snapshots, SQLite, React, Zustand, Vitest.
**Spec:** ENVIRONMENT_DESIGN.md (including review rounds 3–5).

## Constraints
- Skills only for initial automatic sync; config/instruction files use shared comparisons without automatic sync.
- No remote daemon; transport access must be verified before claiming connectivity.
- Never start stopped WSL during discovery or treat inaccessible scans as deletions.
- Preserve independent edits and full directory revisions; recheck before writes.
- One coordinator; global/project boundaries and explicit project mapping.
- Keep upstream functionality and themes; no mock data in production.

## Review focus
- Stale edits and queued operations must not overwrite concurrent changes.
- Symlink/path traversal must not escape authorized roots.
- Unreachable environments must retain cached inventories and not generate removals.
- Independent local copies must not be conflated or re-enrolled after opting out.
- Partial write failures must retain history and recoverable pending state.

## Tasks
1. [ ] Shared comparison: add `src/lib/text-diff.ts` and `src/components/shared/text-comparison.tsx`; tests for insertion/deletion, CRLF, repeated lines, transfer, dirty/save failure. Add revision-labelled editable linked panes, named keep actions and directory comparison. Integrate Agents and skill review.
2. [ ] Coordinator domain and storage: `crates/hk-core/src/environments/` models, persistent registry, directory snapshots and revisions. Tests with temporary folders for identity, optimistic writes, symlinks, deletion and restore. Public service shared by HTTP/Tauri.
3. [ ] Discovery/transports: local provider paths, WSL stopped/running inventory, SSH/Tailscale discovery and access checks. Tests for argument safety, missing tools and no implicit WSL start. Separate discovered from authorized targets.
4. [ ] Reconciliation: policy/pauses, bidirectional/host-driven preview, conflict detection and winner resolution, ignore removal preferences, stop sync, offline queues, history/variants/backup. Test divergent 3-way edits, stale targets, ignore and disable behavior.
5. [ ] App integration: environment selectors and icons, Sync column after Status, split intentional exclusions, Notifications and shared comparisons, collapsed Settings hierarchy and direct path saves, Marketplace target selection. Preserve existing agent/permission rendering and metadata. Test affected flows.
6. [ ] Cleanup old skill implementation: inspect scratch repository and installed ai-tooling-manager copies; remove only superseded artifacts and pending changes attributable to that solution.
7. [ ] Verify: focused tests per task, full frontend suite/build and Rust service suites once, review integration and report unavailable transport/platform checks explicitly.

## Execution ledger
- Prepared plan after inspecting upstream v1.17.1. No prior multi-environment production code exists.
- Rust toolchain installed for backend verification; no application settings or skills modified by installation.
