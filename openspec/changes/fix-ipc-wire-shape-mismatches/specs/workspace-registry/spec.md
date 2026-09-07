## MODIFIED Requirements

### Requirement: Presentation Fields on Listed Workspaces

The data returned by the workspace listing command and the aggregated repo-view
command SHALL include the configured display name (or absent if none), the
colour (or absent if none), and the disabled state for each top-level row. When
no presentation entry exists for a row, the display name and colour SHALL be
absent, the row SHALL be reported as enabled, and consumers SHALL render the row
exactly as they did before the presentation store was introduced.

A field SHALL be considered included only when the frontend can actually read
it: the key the backend emits SHALL be the key the frontend's mirrored type
declares. This holds for every top-level row regardless of how its shape is
expressed in Rust — a struct, or a variant of a tagged enum — so a flat
workspace's display name is subject to exactly the same contract as a repository
group's. Emitting a differently-spelled key SHALL be treated as omitting the
field, because a consumer reading the declared key sees nothing either way, and
the difference is invisible to a compiler on either side of the boundary.

Every key crossing the boundary SHALL be camelCase. Because the Rust types and
their TypeScript mirrors are maintained by hand, with no generated schema, the
application SHALL verify this by inspecting the **serialized output** of the
types the frontend reads, rather than by inspecting their declarations — a
declaration can carry an attribute that does not do what it appears to, and the
resulting mismatch is invisible to both a Rust test that builds values in Rust
and a TypeScript check that compares the mirror against itself. The verification
SHALL descend into nested objects and arrays, since a correctly-spelled outer
shape can contain a wrongly-spelled inner one.

#### Scenario: Workspace list includes display name and colour

- **WHEN** the frontend requests the list of registered workspaces
- **THEN** each workspace entry includes its configured display name (or null), colour token (or null), and disabled state
- **AND** workspaces with no presentation entry return null for name and colour, and report as enabled

#### Scenario: Repo view includes display name and colour for the group

- **WHEN** the frontend requests the aggregated repo-and-flat view
- **THEN** each repo group entry includes its configured display name (or null) and colour token (or null)
- **AND** flat workspace entries in the same view also include their per-workspace display name and colour

#### Scenario: A flat workspace's display name survives the boundary

- **WHEN** the user sets a display-name override on a flat (non-git) workspace
- **AND** the frontend requests the aggregated repo-and-flat view
- **THEN** the flat entry carries that override under the same key the frontend's mirrored type declares
- **AND** the tree row, the header label, the file-browser label and the reader title all render the override rather than falling back to the folder's basename

#### Scenario: Every key crossing the boundary is camelCase

- **WHEN** the types the frontend reads are serialized
- **THEN** no key at any depth of the resulting payload is spelled in snake_case
- **AND** this holds for the fields of a tagged enum's struct variant as much as for a plain struct's fields

#### Scenario: Listing reports disabled workspaces so Settings can render the toggle

- **WHEN** a workspace is disabled and the frontend requests the list of registered workspaces
- **THEN** the workspace is present in the response, marked disabled
- **AND** it is not omitted from the listing the way it is omitted from the tree pane's aggregated view
