# Editable merge implementation plan

**Goal:** Implement the approved source/result/source comparison with accept and ignore controls, editable results, and up to three selectable sources.
**Architecture:** Shared React editor; immutable source snapshots and block-aligned drafts. Preserve existing revision-checked save APIs. Writes are explicit and sequential; partial failures retain successful revisions and remaining draft.
**Spec:** Approved `mockups/agents-idea-diff-prototype.html` and conversation: × › on left, ‹ × on right. Sources stay read-only; result accepts multiline edits.
**Constraints:** Preserve existing work. No commits. No server or dependencies for mockups. Historical sources cannot be written. Do not claim atomic multi-environment writes.

- [x] Add and test a three-source alignment model preserving exact text, line endings, insertions and deletions.
- [x] Replace shared text comparison with aligned source/result/source blocks, word highlights, accept/ignore, manual resolve, undo/redo, navigation and explicit save.
- [x] Extend file comparisons with optional third source and revision-safe saves; retain notification integration.
- [x] Test failed/partial writes, historical sources, ignored changes, multiline drafts, source switching and unchanged-source keeps. Build and run frontend tests.

Review focus: empty files, overlapping third-source changes, duplicate file IDs, source read-only state, external changes during save.

Verification: 437 frontend tests / 59 files passed with `npm test -- --maxWorkers=2`; production build passed. Default parallel run had one worker exit unexpectedly; bounded-worker rerun passed. Review findings fixed: lock comparison reset controls during apply, stop later writes after unmount, preserve CRLF on textarea edits. Running app serves the rebuilt assets. No backend behavior changed.
