## MODIFIED Requirements

### Requirement: File Browser Surface

Clicking a top-level Repo group row or a flat workspace node in the tree SHALL render the workspace file browser in the detail pane, replacing the pane's current contents (artifact, commit, or Dashboard). The browser SHALL present two regions: a navigable folder tree of the workspace's markdown files, and a read-only preview that renders the selected file with the same markdown renderer used for change artifacts. For a Repo group the browse root SHALL be the repository's main worktree; for a flat workspace it SHALL be the workspace folder itself. Opening the browser SHALL dismiss the Settings and Archive panes, and SHALL NOT alter the commit rail's existing re-scoping behaviour for the clicked row. The browser is read-only: it SHALL NOT provide any editing affordance.

While a file is selected, the preview region SHALL display that file's **root-relative path** above the rendered markdown, so the preview identifies what it is showing. The path SHALL be shown in full, exactly as it addresses the file beneath the browse root, with no leading separator and no truncation. The browser is workspace-scoped and has no change context of its own, so the path — not a change name — is the identity available here; for a file under `openspec/changes/`, the path contains the change's directory name.

The path SHALL **copy itself when clicked**, exactly as specified by the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability: a single primary click SHALL place exactly that path on the clipboard and select it as confirmation, the outcome SHALL be indicated and announced, a refused write SHALL leave the value selected, and the path SHALL be keyboard-activatable as a tab stop without introducing any global chord. (This supersedes the previous contract, which specified selection only and forbade an application clipboard write.)

The preview's path sits below the browser's own header rather than flush with the top of the window, so it takes no titlebar-strip clearance.

When no file is selected, the preview region SHALL continue to show its existing empty state and SHALL render no path.

#### Scenario: Repo group click opens the file browser

- **WHEN** the user clicks a top-level Repo group row
- **THEN** the detail pane renders the file browser rooted at the repository's main worktree
- **AND** the commit rail re-scopes to that repository exactly as before this feature

#### Scenario: Flat workspace click opens the file browser

- **WHEN** the user clicks a top-level flat workspace node
- **THEN** the detail pane renders the file browser rooted at the workspace folder

#### Scenario: Selecting a file renders its markdown

- **WHEN** the user selects a `.md` file in the browser's folder tree
- **THEN** the preview region renders that file's markdown with the same renderer used for artifacts

#### Scenario: Preview names the selected file's path

- **WHEN** the user selects a `.md` file in the browser's folder tree
- **THEN** the preview region shows that file's root-relative path above the rendered markdown
- **AND** the path is shown in full, with no truncation

#### Scenario: A change artifact's path carries the change directory name

- **WHEN** the selected file is an artifact under `openspec/changes/<name>/`
- **THEN** the displayed path contains `<name>`, the change's directory name

#### Scenario: One click copies the whole path

- **WHEN** the user clicks once on the displayed path
- **THEN** exactly that path is placed on the clipboard
- **AND** it is also selected, as confirmation of what was copied
- **AND** the outcome is indicated and announced

#### Scenario: No file selected shows no path

- **WHEN** the file browser is open and no file has been selected
- **THEN** the preview region shows its empty state
- **AND** no path is displayed

#### Scenario: Opening the browser dismisses modal panes

- **WHEN** the Settings or Archive pane is open and the user clicks a top-level row
- **THEN** the pane closes and the detail pane shows the file browser
