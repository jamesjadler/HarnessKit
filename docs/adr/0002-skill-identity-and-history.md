# Separate skill identity, installation identity, and revisions

Give each skill or named variant a stable identity shared by its synchronized
copies, and give each installation a separate identity. Names and paths cannot
serve as identity because skills can move or be renamed and unrelated skills
can have identical names.

Record complete skill-folder revisions, with revert creating a new revision.
Variants retain their origin but evolve independently. Retain history and removed
skills until explicit pruning or permanent deletion to keep synchronization and
removal recoverable. A/B testing execution is outside the tool's scope.
