# Skill sidebar implementation

Approved direction: keep configuration/instruction controls in Agents; move managed skill files into the selected Extensions skill sidebar, retaining direct editing, save/sync, comparisons, history, restore and variants. Variants and its inline Create variant control sit beside paths/history as a collapsed section.

1. Test selection by exact skill directory and agent, including nested files, excluded sibling paths and independent variants.
2. Add shared selection helper, filter skills out of Agents, reuse managed rows inside a nested skill file tree in Extensions. Keep legacy file browsing for unmanaged files.
3. Preserve managed file IDs and backend commands; no migration or data deletion. Refresh inventory while Extensions is visible.
4. Run focused and full frontend tests and production build; commit and refresh served frontend.

Implemented with shared ManagedFileRow controls and exact directory/agent selection. File variants remain independently editable; whole-skill duplication remains available inside the main skill file's Variants section. Unmanaged skills retain existing file browsing. Verification: 455 frontend tests passed across 62 files, TypeScript/production build passed, app returned HTTP 200 on port 7070. Backend persistence and comparison commands unchanged.
