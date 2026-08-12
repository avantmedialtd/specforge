## REMOVED Requirements

### Requirement: Monthly Season Model and Deterministic Naming

**Reason**: The `seasons` capability is retired in full. The monthly window, the season index, the generated names, and the launch-relative numbering have no remaining consumer.

### Requirement: Two-Track Progression — Resetting Season, Permanent Career

**Reason**: Both tracks are removed. The resetting seasonal track goes with the battle pass; the permanent career tier goes with it rather than surviving as a standalone ladder, so `seasons.rs` can be deleted whole. The streak — the one line this requirement declared a career fact — survives independently under the `dashboard` capability's *Streak and Contribution Heatmap*.

### Requirement: Season Score Derivation

**Reason**: There is no season score to derive. The underlying activity-log events and commit mining are unaffected and continue to feed the progress layer.

### Requirement: Battle-Pass Tier Ladder and Named Bands

**Reason**: The battle pass is removed; there are no tiers or bands.

### Requirement: Adaptive Pacing with an Overflow Lane

**Reason**: With no completion total to pace, the entry baseline and overflow lane are removed. The trailing-average method itself survives privately in the Dashboard's today-versus-average comparison.

### Requirement: Rotating Generated Objectives

**Reason**: The objective archetypes, their deterministic rotation, and their bonus scoring are removed.

### Requirement: Procedural Badge Treatments

**Reason**: Badge finishes are removed permanently, including the generator, rarity ladder, and generator versioning.

### Requirement: Treatment Locker, Equipping, and Soft-FOMO Vault

**Reason**: The locker, the equip action, and the vault rotation are removed. The persisted `season` block in the settings file becomes an orphaned key that deserializes past and is dropped on the next write; no migration is performed and unlocked treatments are discarded silently.

### Requirement: Silent Backfilled Seasons

**Reason**: There are no seasons to reconstruct. Git backfill of the activity log itself is unchanged and remains specified by the `activity-log` capability.

### Requirement: Season Rollover and Recap

**Reason**: There is no boundary to cross, no recap to mint, and no rollover bookmark to persist.

### Requirement: Read-Only and Offline Operation

**Reason**: Scoped to season computation, which no longer exists. The equivalent guarantees for the surviving surfaces are already carried by the `dashboard` capability's *Read-Only Operation* and the `commit-garden` capability's *Read-Only Graphs*.
