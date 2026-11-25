# All-file versions and coalesced Agents implementation

**Goal:** Ship approved agents-file-versions-prototype.html interactions against real data.
**Architecture:** Persistent coordinator-owned managed file inventory with immutable revision contents, environment copies, named independent duplicates, CAS edits/acceptance and per-destination sync results. Shared frontend file detail uses one logical row, direct editing, collapsed paths/history/variants, common diff icon. Existing skill/extension behavior preserved.
**Spec:** ENVIRONMENT_DESIGN.md and mockups/agents-file-versions-prototype.html; final Compare icon and Not-synced copy in agents-file-controls-review.html.

- [x] Backend: inventory all available agent config/instruction files across environments, persist revisions/variants, command save/accept/restore/variant/retry, discover changes without accepting automatically, sync healthy enabled equivalent copies with CAS. Cover no-ops, failed writes, background acceptance, variant isolation, project identity and unavailable targets.
- [x] Frontend: managed file store/types and single-agent grouped rows with env icons, content always editable; paths/history/variants collapsed; compact Copy/Compare icons, Restore link with confirmation; explicit save, pending change accept, retry. Preserve scope filtering/deep links and unrelated custom path flows.
- [x] Extensions: named variant creation available in Extensions as well as Agents. New ID and requested name; no original overwrite.
- [x] Shared compare: two source entries use two editable panes; third source/explicit merge result can use three. Same-content details remain readable/editable outside diff.
- [x] Review, focused/full relevant tests, build and relaunch app for review.

Constraints: commit tested milestones on feat/multi-environment-skills in /home/demonlord721/code/HarnessKit (user confirmed repository and branch). No destructive cleanup. Retain prior dirty work. No unrelated-agent or basename-only project grouping. No inferred equality for inaccessible copies. Failed/no-op writes do not increment. Accepting real background content records exactly one revision before sync attempts; conflicts remain visible. Variant copies have independent IDs and revision streams. Restore records new revision, never deletes history.


Verification (2026-11-24): frontend 452 tests across 60 files passed; core 887 unit tests plus 17 integration tests passed (one ignored). Production frontend and hk-cli builds passed. Live authenticated inventory returned 509 files across 10 agents, including 65 records with multiple environment copies. App relaunched on 127.0.0.1:7070. No live source files were edited during verification.

Current boundaries: project files retain environment-specific project identities; automatic cross-environment project linking is not implemented. New managed-file variants are coordinator-owned duplicates with independent history and no assigned environment copies. Extension variant creation requires an existing tracked snapshot or inventoried managed file. Remote write behavior is covered by tests, not a live MacBook write check.
