## ADDED Requirements

### Requirement: Overlapping Graph Refreshes Settle on the Newest

The rail's live refreshes (see the *Live Graph Updates* requirement) SHALL be coalesced. Ref-change notices for the shown repository that arrive while a read is pending or in flight SHALL cause at most one further read after it. The rail SHALL apply only the reply to the newest read it requested for the shown repository and window size. An older reply SHALL NOT replace a newer graph, even when it arrives last. A reply for a repository or window size that is no longer shown SHALL be discarded.

A refresh SHALL NOT present the rail's loading state while a graph is on screen. The loading state is for a rail with no graph yet. A failed refresh SHALL behave as a failed read does today.

#### Scenario: A burst of ref changes costs one more read

- **WHEN** three ref-change notices for the shown repository arrive while a graph read is in flight
- **THEN** exactly one further read follows the one in flight
- **AND** the rail shows that read's graph

#### Scenario: A slow older reply does not win

- **WHEN** two graph reads for the shown repository overlap and the older one replies after the newer one
- **THEN** the rail keeps the newer read's graph

#### Scenario: A refresh does not flash the loading state

- **WHEN** a graph is on screen and a ref change triggers a refresh
- **THEN** the rail keeps showing the current graph until the refreshed one replaces it
- **AND** the loading state is not shown
