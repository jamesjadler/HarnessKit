# HarnessKit multi-environment design

Status: shared understanding confirmed by the user; static visual review in progress.

## Purpose

Extend the existing HarnessKit fork to manage coding-provider assets across
Windows, WSL, Linux, and macOS, with Tailscale connectivity, automatic discovery
where supported, and reversible skill synchronization. Preserve HarnessKit's
existing capabilities while making environment context apparent throughout its views.

## Architecture and alternatives

The user selected one coordinating HarnessKit instance, able to run with its
window closed, rather than installing a HarnessKit service in every environment.
Prefer SSH over Tailscale for remote access and Windows host commands for WSL.
Remote access may require SSH setup; Tailscale network discovery alone does not
grant filesystem access. This transport approach needs an integration feasibility
check before production implementation.

Per-environment services could provide local watchers and independent execution,
but conflict with the installation preference. Depending on T3 for initial
operation would avoid some connection setup but relies on an unverified app-facing
integration. Standalone support comes first; T3 environment import follows.

The coordinator must be running and awake for synchronization. No automatic
failover or concurrent coordinators are promised by this initial design.

## Machines, environments, and scope

- A physical machine groups multiple environments, including its native OS and
  individual WSL distributions. Providers and projects belong to environments.
- Discover candidates automatically where possible. New environments participate
  under the configured sync policy once authorized access is available, unless
  disabled. Show Setup required until that access works.
- Never automatically start a stopped WSL distribution for scanning or syncing.
  Show Stopped — sync unavailable and an explicit Start to sync action.
- Keep global and project assets separate. Sync between corresponding projects
  requires explicit mappings rather than matching folder names.
- Retain unavailable environments' cached inventories and scan timestamps; show
  Offline, WSL stopped, Setup required, or Access denied as applicable. Distinguish
  cached information from successfully verified current contents.
- Retain source provenance, provider, scope, environment, paths, and file contents.
  Environment context applies throughout existing views, including marketplace,
  settings, history, and comparisons. Preserve upstream layouts and behavior wherever possible to minimize maintenance. Exact layouts require visual approval. All additions respect the existing Tiesen/Claude themes and System/Light/Dark modes using application appearance tokens.

## Synchronization

- Initial synchronization covers skills, including their supporting files.
  Other asset types retain inspection and targeted marketplace installation.
- Default policy is All, permitting eligible cross-provider sharing. Like providers
  remains available; None disables automatic sync; path-limited policies specify participating
  folders and explicit cross-platform source/destination mappings.
- Machine, environment, project, and skill overrides refine the default. More
  specific rules win, but an explicit machine/environment pause stops all syncing
  for that target. Display effective policy and where it comes from.
- Changes may originate in any participating environment. Initial reconciliation
  combines skills and copies missing entries. Independent divergent contents
  require user resolution; never silently select a winner.
- Manual mode uses the same rules and waits for Sync now with a change preview.
  Automatic mode applies eligible non-conflicting changes while respecting pauses.
- Recheck contents before applying queued work after reconnect. Show per-target
  completion, pending work, and errors; pending operations can be cancelled.
- Enable/disable and explicit removal follow the same sync policy as edits.
- Removal is recoverable, with history retained in a Removed view. Restore creates
  a change that follows the policy, including delivery to returning environments.
- Confirmed external deletion of tracked entries defaults to Remove and sync.
  An opt-in recovery-prompt setting instead asks Remove and sync or Recover and
  holds propagation pending that decision. Failed scans, inaccessible directories,
  and offline/stopped environments are never deletion evidence.
- A removal versus an independent edit is a conflict requiring user intervention.

## Identity, revisions, and variants

- A stable skill/variant ID is shared across synchronized copies. Each installation
  has a separate ID. Name/path changes do not themselves change skill identity.
- Match unmanaged skills using source repository and skill path where available.
  Ask about uncertain matches rather than conflating unrelated same-named skills.
- Version the entire skill folder. Capture HarnessKit edits and observed external
  changes; changes made between scans cannot be reconstructed.
- Reverting creates a new revision and retains intervening history. It propagates
  through the normal policy rather than leaving an unexplained local divergence.
- A named variant starts as a complete copy with its own identity, history, and
  sync membership, retaining its original skill and starting revision. Original
  updates do not overwrite variants. Original and variants may coexist under
  distinct installed names.
- Compare with selects original or variant and supports revision-to-revision
  comparison. Original defaults to its latest revision. Include supporting files;
  do not assume every asset is text. Comparison does not itself merge changes.
- A/B execution, deployment experiments, and scoring are outside this tool's scope.
- Store IDs, history, and sync records in coordinator-owned storage, outside
  installed skill folders. Provide backup/export and restore. Retain revisions
  and removed skills until explicit pruning/permanent deletion; show storage use.

## Marketplace

Select environments, providers, and global/project destinations. All environments
expands into a visible target list. Show incompatible targets with explanations;
queue unavailable targets. Preview the broader effects of the applicable sync
policy, which can propagate a skill beyond the initial installation targets and
to newly discovered participating environments later.

## Implementation decisions still to validate

These are engineering work, not additional user commitments: discovery APIs,
authorized SSH access and Windows-to-WSL execution, provider compatibility,
history storage format, atomic writes, race detection, scan scheduling, path and
symlink handling, credential storage, interrupted-job recovery, and backup format.
Do not claim transport feasibility until tested. Surface material compromises to
the user rather than silently introducing per-environment services.

## Review sequence

Confirm this shared understanding, then produce static mockups in HarnessKit's
existing design language. After visual approval, create a separate throwaway
interactive prototype. Implement production UI and integrations only after the
required interaction approval. Independent backend feasibility work may proceed
when authorized and independent of unapproved UI choices.

Supporting decisions: docs/adr/0001-single-coordinator.md and
docs/adr/0002-skill-identity-and-history.md. Vocabulary: GLOSSARY.md.

## Visual review corrections (supersede the first mockup)

- Preserve Extensions' Install New, Check Updates, existing filters, and table
  columns. Add an Environments column and per-skill out-of-sync state with Review
  and finish. Remove the mockup's extra global/project filter and grouping; retain
  the existing sidebar scope control and underlying scope separation.
- Retain the notification banner/toast. Add a Notifications section collecting all
  decisions, with direct access to the affected skill and resolution. Waiting for
  an offline target alone is activity, not a decision requiring user intervention.
- Use red/green text diffs for simple discrepancies; provide file-by-file review
  for multi-file conflicts. Preserve the coordinator information.
- Environment configuration belongs in Settings' Environment Manager, with Agent
  Paths grouped under environments while retaining existing path controls. Keep
  the existing Settings formatting and appearance controls.
- Keep Marketplace's existing list/detail layout. Add environment (including All)
  and global/project selectors within its Install to Agent section. Connection
  configuration remains in Settings.
- Pause controls explicitly identify the environment or physical machine affected;
  remove the ambiguous global Pause sync control.
- Narrow stacked layout is accepted as a direction; add easy access to Notifications.
- For browser review, provide a direct absolute HTML file link. The user opens it
  in T3's browser with the annotator; do not start a mockup server.

## Visual review round 3

- Preserve original agent mascot icons and permission icons/tooltips; prior text
  substitutions were mockup shortcuts, not approved product changes.
- Add Sync immediately to the right of Status. Synced, queued, and decision states
  belong there. Use one actionable "Conflict · user attention needed" label to
  open resolution, rather than separate warning and Review/finish links.
- Show comparisons directly for out-of-sync notifications, including environment,
  provider, absolute file location, and revision on each side. Explicitly identify
  revision lag versus independent edits; missing-file views identify who removed
  the file. Red/green labels correspond to the compared sides, with explicit text
  attribution rather than relying on color to explain edit provenance.
- Multi-file comparisons use a hierarchical directory diff with file states.
  Selecting a text file leads to its content diff; binary changes need inspection.
- Combined Environments & Agent Paths is the top Settings section. Group machines,
  their environments, provider paths, access status, and machine/environment/provider
  sync pause state together. Add/discover controls and connection setup prompts
  appear at the bottom of this section.
- Keep the approved Marketplace environment dropdown and existing upstream layout.
- Sync mode includes independent timing (automatic/manual) and direction
  (host-driven/bidirectional) choices. Bidirectional remains the existing default.
  Host-driven needs an explicit source; remote-change conflict treatment is still
  to be finalized before implementation and must not imply silent data loss.
- Diff editing should support linked scrolling, editing either side, per-change
  transfer arrows, Keep left / Keep right, and explicit saving. This remains a
  static visual proposal; actual interaction follows visual approval.

## Shared comparison experience

All agreed text-diff behavior applies across the application, not only to skill
conflicts or variants. Use the same comparison component for config.toml,
AGENTS.md and other instruction files, skill files, and other text assets,
including comparisons opened from the Agents view. Preserve environment/provider
identity, absolute paths, revision identifiers where available, explicit
comparison direction, red/green differences, linked scrolling, editing either
side, per-change transfer, Keep left/Keep right, and explicit save actions.
Do not fabricate revision history for assets without recorded revisions; label
those sides as current or cached snapshots with capture times. Multi-file
comparisons use the same directory tree and open text files in this shared viewer.
This broadens comparison UI coverage, not the initial skills-only automatic
synchronization scope.


## Review revision 4 — environment choices and sync tree

- Add quick Keep environment + revision choices to notifications and directory comparisons; folder-wide and file-only scopes are explicit.
- Show three-or-more divergent environments in a file-tree matrix against the common base, with yellow unresolved conflicts and selectable pairs for text comparison. All participants must be resolved before finishing. Apply the same comparison conventions to config.toml, AGENTS.md, skills, and text comparisons in Agents.
- External-removal prompts offer Review, Sync with other envs (propagate removal), and Ignore (acknowledge intentional local removal, preserve other copies, prevent repeat alerts or automatic recovery). After Ignore, offer just once or always ignore external skill removals. Store the latter as a user preference editable in Settings; it overrides the removal policy only for this event category.
- Distinguish machine headers, indented environment containers, and nested providers. Each has one right-aligned sync status/control; paused items use a yellow resume control. Stopped environments remain distinct from paused syncing.
- Remove path edit icons. Paths are edited directly and saved when Settings closes or focus leaves the tree; preserve validation errors and unsaved values if saving fails.
- Revision 4 remains visual-only: controls depict intended behavior without implementing interaction or production changes.


## Review revision 5 — icons, independent copies, collapsed hierarchy

- Environment cells use OS/environment icons with hover descriptions and accessible names, including machine identity where needed to disambiguate. Green indicates synced; dimmed icons indicate no sync; yellow indicates attention or waiting; red indicates a blocking error. State descriptions distinguish causes.
- Sync column uses a green check for synced, yellow queued/conflict states, neutral intentional exclusions, and red failures.
- Split one skill into synced and intentionally unsynced inventory rows, each listing only its own environments. Preserve shared identity and existing metadata; no false conflict for independent local copies. Plugin synchronization remains outside initial scope.
- Add Stop syncing this skill alongside environment winner actions. This turns off skill-wide synchronization, preserves all current environment copies, and clears the sync conflict without choosing or overwriting a winner. Re-enabling sync requires a fresh comparison.
- Machines and environments are independently collapsible, both collapsed by default. Keep aggregate status visible while collapsed. Static revision 5 shows default, machine-expanded, and environment-expanded states; interaction remains for the next approved stage.

## Follow-up requirements — consistent agent catalog and path groups

- Use the same supported-agent catalog and agent-home presentation in every environment. Distinguish detected/configured paths from supported but unconfigured agents; a missing optional installation is not a sync failure.
- Within each environment, split entries into **Tracked paths** and **Untracked paths**. Each group is independently collapsible, in addition to the existing machine/environment collapse controls. Group headers retain entry counts and issue counts while collapsed.
- When an environment is opened, default **Tracked paths** to expanded and **Untracked paths** to collapsed. Both remain independently toggleable.
- Tracked means enrolled in HarnessKit tracking, not necessarily actively syncing. Paused, disabled-for-sync or temporarily inaccessible tracked paths remain in Tracked paths; a failure must not silently move an entry into Untracked paths. Supported/default paths not enrolled in tracking belong under Untracked paths and show their detected or not-configured state.
- Put specific path/agent-home failures and the relevant next action directly on the affected entry. Healthy entries retain their normal state.
- If some participating agents fail while others remain usable, show **Partial sync · N agents need attention** at the environment level. Keep comparisons and policy-permitted synchronization available between healthy participating agents. Preserve an explicit paused state when syncing is paused; partial availability does not implicitly resume it.
- Collapsing a group changes only visibility, never tracking, participation, pause state or comparison eligibility.
- This is a recorded requirement for the next UI revision, not an implemented control change.

## Implemented follow-up — path groups and connection access

- Enrolled provider paths remain in Tracked paths regardless of pause, disabled agent or access failure. Tracked starts expanded; Untracked starts collapsed. Catalog entries expose supported agent homes separately from enrollment.
- Partial availability is yellow and does not mean the user paused synchronization. Explicit pause remains a separate control. Missing folders are yellow on their own entry; troubleshooting and retry are scoped there.
- Available skill comparisons remain accessible and choose healthy copies first. Unavailable snapshots are labeled cached and read-only.
- Empty-folder recovery is explicit, non-recursive, and guarded against prior installed content/history. It does not modify account permissions.
- Docker connections reuse an available context, pin a container identity, require Python 3, and never start a stopped container. Shared bind/volume paths are rejected rather than synchronized to themselves. The user can map the host folder once instead. This initial safeguard is intentionally conservative.
- VM access uses existing SSH aliases or accessible mounted directories. Hypervisor discovery/provisioning and lifecycle controls are outside this pass.

## Nested agent path revision — implemented

Agent homes, global skills directories and project directories now live inside each environment/agent. Each entry has independent enabled state and editable path. Tracked sections start expanded and untracked sections collapsed; access diagnostics run independently of pause. Settings uses the available horizontal width and consistent SVG pause/play controls.

Access results distinguish empty, missing (yellow), permission denied (red), other error and unchecked. Counts reflect skill folders, not arbitrary filesystem entries. Local paths are checked during reconciliation; remote paths are checked in a single batch when opened/refreshed/edited. New untracked path edits do not enroll them. Project additions stay scoped to the selected environment and agent. Existing registered local projects are retained for agents whose markers are present.

Tracked path relocation pauses synchronization to prevent first-observation content from overwriting peers. Editable drafts preserve their original CAS baseline through remounts. Native local browsing is available in the desktop app; web/remote project additions use explicit destination paths.

## Pending visual revision — picker hierarchy feedback

Use fully styled, standalone graphical mockups with explicit fallback theme colors; do not rely on injected preview CSS. Enabled/disabled controls precede titles. Project add field, directory picker and Add button sit inline with Projects. Enabled projects belong under Tracked, disabled projects under Untracked (this supersedes the earlier project grouping rule). Keep directory pickers for every access state and base paths inline with agent titles. This revision remains static pending review.

- Latest simplification: no Add buttons for path selection. A successful picker selection adds immediately; typed input submits on Enter or blur after validation, with inline errors and preserved drafts on failure. Avoid duplicate submission on Enter followed by blur.

- Removal controls on every entry remove its registration, never filesystem content. An × at the left inside each path field clears the draft for replacement, not removal. Project addition requires recognized files for the relevant agent and discovers supported project components (skills, configuration, instructions, hooks) as the original scanner does. Projects with no recognized content are rejected with an inline explanation.

- Clear-control correction: × belongs on the right inside each path input and is visible only while that input has focus. Picker and Remove remain visible separately.
- Project discovery is recursive: selecting ~/code discovers valid child projects and their supported components, skips irrelevant dependency/build/VCS folders, avoids symlink loops, and deduplicates existing registrations.

- Project component rows must be visibly indented beneath their project root/header, with a tree guide and subordinate background rather than aligned as siblings.

- Projects subgroup is visibly nested below its Tracked/Untracked parent, followed by project roots one level deeper and discovered components another level deeper. Agent-scoped discovery ignores other agents; standalone environment project discovery automatically categorizes each agent’s matching data, recursively and without duplicates.


Approved visual direction: Quiet tree (A), with explicit machine → environment → agent → tracked/untracked → projects → components containment. Claude Code belongs to Ubuntu/WSL; Windows is a sibling environment under the desktop; MacBook Air is a separate remote machine. Interactive review artifact: mockups/environment-a-prototype.html.

Sync health only includes paths the user intends to track. Missing, denied, or otherwise unavailable untracked paths may show their local diagnostic, but must not make an agent, environment, machine, or overall sync partial. An unavailable tracked path does contribute an issue, while healthy tracked paths remain usable.

### All-file versions and coalesced Agents inventory (2026-11-24)

Required: version history and named variants apply to every managed file, not only skills. The Agents sidebar remains one entry per agent. Within that agent, coalesce equivalent files by agent, category, linked scope and logical relative path. Show an environment icon column and a sync status. Expanding a file immediately shows every environment's full path. Verified equal contents in the same variant are Synced and need no diff; unavailable copies must not be presumed equal. Distinct intentional variants retain independent histories and are not conflicts with one another.

Visual proposal: `mockups/agents-file-versions-static.html`. Version restore creates a new revision; named variant assignment selects environments. These controls are a proposal pending visual and interaction approval, not implemented backend capabilities.


2026-11-24 follow-up: file name, environment icons and sync status stay on one row. Selecting a fully synced file still shows text content plus all paths, each with Copy path. Both Agents and Extensions expose Create variant; it requires a user-supplied name and creates a duplicate with a new identity, never an alias of the original. Each successful app-managed edit increments the version; failed saves and no-op saves do not. Proposed new duplicate starts at r1 with recorded source lineage. Visual mockup includes the shared naming dialog; no production changes yet.

Follow-up layout: Environment paths, Version history, and Variants belong inside the expanded file above its always-visible editable contents. Paths and history default collapsed. No separate Edit mode; content accepts typing directly. Compare and Restore actions have spacing without a dot separator. Two-version comparisons use two panes; a third pane is only needed for a third version or an explicit merge result.

Version history actions: compact icon-only Compare with tooltip/accessible name; Restore styled as a text link with an explicit confirmation stating that it creates a new version and attempts sync. Cancel must leave content and history unchanged.

Use the approved split-document / dotted-divider Compare icon consistently for file rows, version history, variants, and background-change review in Agents and Extensions; no alternate arrows or text-only Compare buttons. Keep accessible action labels and tooltips.

File copies with differing contents use the status label “Not-synced”, not “Different”.
