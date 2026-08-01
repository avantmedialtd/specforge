## ADDED Requirements

### Requirement: Preview Link Handling

Markdown rendered in the file browser's preview SHALL be governed by the *Link Handling in Rendered Artifacts* requirement of the `spec-browser` capability, with the authorized browse root serving as both the resolution base's root and the containment root for relative file links. The browser's enumeration and read contracts are unchanged: listings remain markdown-only and the guarded read remains markdown-only — a linked `.html` file is opened through the validated open operation, never listed or read through the browsing surface.

#### Scenario: A mockup link in a previewed file opens

- **WHEN** a previewed markdown file in the file browser contains a relative link to an `.html` file that exists inside the authorized browse root
- **THEN** clicking the link opens the file via the operating system's default handler
- **AND** the preview pane neither navigates nor blanks

#### Scenario: Preview links under an accepted main worktree open

- **WHEN** the browse root is a repository main worktree accepted because a worktree of that repository is registered
- **THEN** a valid mockup link in a previewed file opens rather than being refused
