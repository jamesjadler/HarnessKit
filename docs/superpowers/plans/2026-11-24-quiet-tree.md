# Quiet Tree Implementation Plan

> Use subagent-driven-development for the independent backend task and review; integrate frontend in this session. Existing dirty feature checkout is the integration workspace: preserve all prior work, do not commit unrelated changes.

**Goal:** Implement approved A static and interactive review using real environment data.
**Architecture:** Existing coordinator registry remains authoritative. Extend path operations and scoped discovery in Rust/Python transport; React renders machine/environment/agent/path nesting and uses real browse operations. No mock data in production.
**Tech Stack:** Rust, existing Python remote helper, React/TypeScript, Tailwind, Vitest.
**Spec:** ENVIRONMENT_DESIGN.md; mockups/environment-alternative-a.html; mockups/environment-a-prototype.html.

## Global constraints
- Machines contain environments; agents remain inside their environment. Remote machines are separate peers.
- Tracked open / Untracked collapsed by default. Projects are nested under the corresponding group; disabled projects move to Untracked.
- Access errors in untracked/disabled paths do not contribute to partial sync. Track intent survives temporary missing/denied access. Permission denied red, missing yellow. Healthy paths remain usable.
- Enable before name; base path inline; clear on right only while focused; pickers available even on inaccessible paths. No Add button; pick or Enter/blur validates and adds once.
- Recursive project discovery is bounded, skips build/dependency/VCS trees and symlinks, rejects folders without recognized data, deduplicates. Agent-scoped discovery ignores other agents; environment discovery categorizes into matching agents.
- Remove forgets configuration and never deletes files. Survives refresh; no reappearing auto-discovered entry.
- Existing CAS path-save, pause, backup, and connection safety retained. No automatic writes caused by browsing/discovery.

## Tasks
- [x] Backend: real browse command and recursive discovery, component metadata, registration removal and tracking semantics. Regression tests for recursive scoped discovery, no files, dedupe, removal persistence, inaccessible tracked paths.
- [x] Frontend: quiet-tree hierarchy, reusable path control/picker, automatic addition, nested groups, status aggregation, independent controls. Tests for save/picker, enable/disable group movement, hierarchy, and untracked errors.
- [x] Integration: review changed files, run appropriate frontend/backend suites and builds, restart local review server, verify response. Record limitations accurately.

## Review focus
CAS drafts on remount; absent paths mistakenly treated as deletions; removed catalog rows reappearing; remote discovery path semantics; disabled project children affecting aggregate health; Enter+blur duplicate creation.

## Verification

- Frontend: 420 tests passed in 58 files; production build passed.
- Backend: 914 tests passed, one live SSH test ignored; CLI build passed.
- Review fixes: canonical catalog identity lookup, picker portal no premature draft save, failed remove retains draft, empty skills-only projects rejected, all component tombstones cleared on explicit project readdition.
- Live local API: Ubuntu picker ~ resolves /home/demonlord721; Windows picker ~ resolves /mnt/c/Users/demon; all 68 catalog entries carry kind/tracked/is_directory metadata.
- Review server restarted at localhost:7070, log /tmp/hk-quiet-tree-server.log. No real SSH/Docker target rerun. Config/instruction comparison API remains existing behavior; new components are detected and editable as paths.
