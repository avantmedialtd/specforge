## MODIFIED Requirements

### Requirement: Tree Expansion Has No First-Sight Auto-Expansion Effect

The application SHALL NOT maintain a separate "first time we see this node, mark it expanded (or collapsed)" code path. A node's default state is derived from its current data on every render — not from any one-shot seeding effect that runs on view changes — and the user's override (if any) is applied on top of that default.

Revealing the node named by an address is not such a code path and is permitted, precisely because it is transient: it SHALL be applied above the override sets without writing them, and SHALL NOT trigger a settings write — see the *Navigation Reveal Is Transient* requirement in the `view-routing` capability. Following a link therefore never rewrites the recipient's stored tree preferences.

#### Scenario: No second-mount re-seeding

- **WHEN** the watcher emits a `cache-updated` event that causes the tree's `views` prop to re-render
- **THEN** the application does not run any effect that mutates the `collapsed` or `expanded` override sets in response to the new view

#### Scenario: User override survives a tree re-render

- **WHEN** the user has expanded an auto-collapsed Section or collapsed a default-open Section
- **AND** the watcher subsequently emits a `cache-updated` event for that workspace
- **THEN** the user's override is preserved after the re-render
- **AND** the application does not flip the node's state as a side effect of the view change

#### Scenario: A navigation reveal leaves the override sets untouched

- **WHEN** the user follows an address naming an artifact whose ancestor nodes they had previously collapsed
- **THEN** those ancestors are shown open so the addressed node is visible
- **AND** the `collapsed` and `expanded` override sets are unchanged
- **AND** no settings write is performed as a result
