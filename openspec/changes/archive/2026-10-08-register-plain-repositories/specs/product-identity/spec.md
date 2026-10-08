## MODIFIED Requirements

### Requirement: OpenSpec Format References Preserved

The application SHALL retain the "OpenSpec" name in every string, identifier, path segment, and error message that refers to the **OpenSpec spec format** as opposed to the SpecForge product. This includes:
- filesystem path segments (`openspec/`, `openspec/changes/`, `openspec/changes/archive/`);
- workspace validation errors;
- file-dialog prompts asking the user to select an OpenSpec workspace or git repository folder;
- any settings copy describing the format the application reads.

Where copy names the two accepted kinds of registration together, it SHALL call them an "OpenSpec workspace" and a "git repository". It SHALL NOT call either a "SpecForge workspace" or a "SpecForge project".

#### Scenario: Workspace folder selection dialog

- **WHEN** the user opens the workspace folder picker from settings
- **THEN** the dialog title refers to selecting an OpenSpec workspace or git repository folder

#### Scenario: Invalid workspace rejection

- **WHEN** the user selects a folder that does not contain an `openspec/` subdirectory and does not lie inside a git working tree
- **THEN** the application emits an error identifying the folder as neither an OpenSpec workspace nor a git repository
- **AND** the error message uses the term "OpenSpec" to describe the required format

#### Scenario: Filesystem layout references

- **WHEN** the application reads or watches workspace contents
- **THEN** it joins paths using the literal segments `openspec`, `changes`, and `archive` as defined by the OpenSpec format
