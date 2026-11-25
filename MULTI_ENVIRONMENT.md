> Implementation status (2026-11-24): the approved design is now implemented in this checkout and verified with frontend, core/web and a temporary MacBook Air SSH test. The sections below preserve the design history. Ongoing SSH access still requires a usable key or authenticated session; the temporary live test session is closed. Automatic sync runs only while HarnessKit runs and applies to skills. Config/instruction comparisons save explicitly. Desktop packaging has not been verified here.

# Multi-environment fork: discovery and requirements

Status: design interview in progress; product implementation has not started.

## New requirements: Tailscale and WSL

User requests multiple environments over Tailscale, WSL support with automatic
discovery where feasible, environment context throughout existing views, clear
accessibility status, manual and automatic synchronization, and marketplace
installation to a selected environment or all environments.

Confirmed in the design interview:
- Each physical machine groups multiple environments, including individual WSL distributions.
- Initial synchronization covers selected skills only; other asset types remain
  available for inspection and targeted marketplace installation.
- Selected sync groups accept edits from any member. Conflicting independent edits
  pause synchronization for the affected skill rather than choosing a winner.
- Tailscale and local WSL management work independently first; T3 integration follows.
- One coordinating HarnessKit instance manages remote access and automatic sync;
  it may run with its window closed. Sync pauses while its machine is asleep.
- Prefer SSH over Tailscale and Windows host access to WSL instead of installing
  HarnessKit services in every environment; feasibility still needs testing.
- Stopped WSL distributions remain stopped and show sync unavailable with a
  "Start to sync" action. Discovery and automatic sync do not start them.
- Offline install targets retain visible, cancellable pending operations. Recheck
  their current contents before applying operations after reconnect.
- Discovered environments automatically join the configured sync policy unless
  sync is disabled. Inaccessible environments show "Setup required"; syncing
  begins once authorized access works, without another enrollment step.
- Default sync policy is "All", sharing eligible skills across providers. Like
  providers remains available; "None" disables automatic sync. Path-limited
  sync selects participating folders with explicit source/destination mappings.
- First sync combines participating skills and copies missing skills. Differing
  contents for the same skill identity require conflict resolution.
- New requested features: skill revision history with revert, and named variants
  derived from existing skills.
- Revision history covers the entire skill folder, including scripts, references,
  and assets. Capture HarnessKit edits and externally observed changes; changes
  between scans cannot be reconstructed.
- Revert creates a new revision and follows the skill's normal sync policy,
  retaining previous history.
- A variant begins as a complete copy, retains its original skill and starting
  revision, and has independent edits, history, and sync membership. Original
  changes do not automatically overwrite it; deliberate comparison/incorporation
  is desired later.
- Originals and variants can coexist under distinct installed names. Each variant
  has a unique identity. Synchronized copies share a skill/variant ID; individual
  installations have separate IDs. Renaming/moving does not change skill identity.
- A/B testing is outside this tool's scope. Variants offer "Compare with original";
  refined to "Compare with" and a selector for the original or a variant, with
  revision selection supporting version-to-version comparisons. Selecting the
  original defaults to its latest revision. Comparison covers the skill folder.
- Global and project skills remain separate. Corresponding projects across
  environments require explicit mappings for automatic synchronization.
- Removal follows the same sync policy as other changes, rather than defaulting
  to installation-local removal. Removal is non-destructive: preserve history in
  a Removed view, and restoration follows the same sync policy, including offline
  members on reconnect.
- A removal conflicting with an independent edit pauses that skill's sync and
  requires user intervention; neither side silently wins.
- Confirmed external deletion of tracked entries defaults to remove and sync.
  Opt-in recovery prompts instead offer remove and sync or recover; hold that
  operation pending the decision. Inaccessible environments, stopped WSL, and
  failed scans are not deletion evidence.
- Enable/disable state propagates according to the same sync policy.
- Previously unmanaged skills use source repository and skill path for matching
  where available. Uncertain matches require resolution; matching names alone do
  not establish identity. Assign stable shared IDs once matched.
- Default sync policy supports machine, environment, project, and individual-skill
  overrides. More specific settings win; explicit machine/environment pause stops
  all its syncing. Show the effective policy and its origin on each entry.
- Marketplace installation selects environments, providers, and global/project
  destinations. Expand "All environments" into a visible target list. Unsupported
  targets explain incompatibility; inaccessible targets queue. Preview subsequent
  policy-driven propagation beyond the initial installation targets.
- Keep revisions and removed skills until explicitly pruned or permanently
  deleted; show storage usage. No automatic history expiry in the first version.

- The coordinator stores IDs, revision history, and sync records outside installed
  skill folders, with backup/export and restore support.
- Manual and automatic sync share matching, conflict, and removal rules. Manual
  mode waits for "Sync now", previews pending changes, and respects pauses.
- Unavailable environments retain browsable last-known inventory, visibly marked
  with its scan time and current accessibility status. Cached data is distinct
  from freshly verified content.

Interview decisions are consolidated in ENVIRONMENT_DESIGN.md for confirmation.
Technical feasibility remains to be validated; product implementation has not
started and this interview does not bypass visual/interaction approval.
Upstream inspected: `12db128adc692aa2136349531f45ebec540431a1`.

## User requirements carried forward

- Manage Windows, WSL, and macOS from one dashboard.
- Later import environment identities, projects, and provider locations from T3 Code
  where supported; T3 is not a dependency of the first version.
- Avoid installing a separate application on every machine where possible.
- Automatically synchronize selected skills between environments.
- Preserve global versus project scope, provider, environment, source provenance,
  and explicit file locations. Separate installations remain separate entries.
- View and edit contents; support reversible disable/removal.
- Retain sensible standalone behavior outside T3.
- Follow visual mockup approval, interactive prototype approval, then UI implementation.

## Existing integration points

- `src/lib/transport.ts` dispatches through Tauri IPC or same-origin HTTP.
  Its single web authentication token and implicit backend assume one node.
- `src/lib/invoke.ts` is the typed frontend command boundary.
- `crates/hk-web/src/lib.rs` starts a node with a local home, database, adapters,
  and optional display name. The README describes separate remote servers and tabs.
- `crates/hk-core/src/store/sync.rs` reconciles scans with local database rows;
  it is not a cross-machine content synchronization engine.
- Existing adapters, scope handling, deployment, and Kit functionality should be
  examined for reuse before introducing another installation format.

## Proposed first slice, not yet an approved design

Introduce explicit environment identity and environment-qualified asset identity,
then a read-only inventory spanning environments. Keep connection discovery and
transport behind an adapter so the core does not depend on private T3 internals.
Follow with a skill synchronization planner before enabling writes.

Proposed synchronization safeguards: policy-controlled membership, content hashes, common
revision tracking, conflict detection, recoverable writes, and offline retries.
Explicit removals and confirmed external deletions propagate by policy, unless
opt-in recovery prompts require a decision for external deletion. Do not copy credentials/configuration as skills.
Automatic synchronization needs an execution host while the UI is closed; a
running agent is not inherently a persistent background service.

## Unresolved feasibility

T3 tools available to the conversational agent do not establish that a standalone
HarnessKit process can invoke them. Verify a supported integration before claiming
T3-based background synchronization or requiring services on remote machines.
Do not copy T3 authentication material or guess private RPC contracts.

## Checkout status

Public upstream cloned locally; working branch `feat/multi-environment-skills`.
GitHub fork: https://github.com/jamesjadler/HarnessKit
`origin` points to the fork; `upstream` points to RealZST/HarnessKit.
HarnessKit v1.17.1 was subsequently installed and launched in WSL at the user's
request. A separate Windows thread was launched for native desktop installation.
This design interview has not deployed remote services or changed skill files.

Visual-review corrections and HTML-link delivery preference are recorded in ENVIRONMENT_DESIGN.md. Revision 2 preserves upstream layouts and adds a Notifications decision section.

Round 3 adds host-driven as an alternative to bidirectional sync, provider-level pause state, a linked editable diff design, and combined environment/path management. ENVIRONMENT_DESIGN.md records the latest visual decisions.

Shared text comparison applies to all text assets, including config.toml, AGENTS.md, skills, and the Agents view; see ENVIRONMENT_DESIGN.md. Initial automatic synchronization remains skills-only.


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
