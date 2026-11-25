# HarnessKit environment management

Language for managing coding-provider assets across machines and environments.

## Language

**Machine**:
A physical computer that groups one or more environments.
_Avoid_: Environment when referring to the entire computer.

**Environment**:
A distinct operating context with its own provider installations, global assets,
and projects, such as Windows or an individual WSL distribution.
_Avoid_: Machine when referring to one context within a computer.

**Sync group**:
A set of skill installations whose changes are shared in both directions according
to a synchronization policy.

**Sync policy**:
The rules determining which environments, providers, and skill folders participate
in automatic synchronization.

**Coordinator**:
The designated HarnessKit instance that manages environments and carries out their
synchronization.

**Sync conflict**:
Independent changes to the same shared skill that require a choice before its
synchronization can continue.

**Skill revision**:
A recorded version of a skill's complete contents, including its supporting files.
_Avoid_: Variant when referring to an earlier version of the same skill.

**Skill variant**:
An independently identified, named skill derived from a particular revision of
another skill, with its own history and synchronization membership.
_Avoid_: Revision when referring to a separately maintained alternative.

**Revert**:
Restoring the contents of an earlier skill revision as a new revision, preserving
the intervening history.

**Skill identity**:
The stable identity of one skill or named variant, shared by its synchronized
copies and independent of its name or location.

**Installation identity**:
The identity of one installed copy of a skill in a particular environment and scope.
