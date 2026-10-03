## 1. App layer and desktop shell: the Settings… menu item

- [x] 1.1 In `crates/openspec-app/src/events.rs`, add `pub const EVENT_OPEN_SETTINGS: &str = "open-settings";` beside `EVENT_TOGGLE_SIDEBAR`. Give it a doc comment in the pane toggles' style: the desktop shell's macOS menu emits it, it is not a `CacheEvent`, it travels the Tauri transport only (browsers own ⌘,) and it carries no payload. In the `tests` module, add `the_open_settings_event_is_its_own_name`, asserting that it:
  - equals the literal `"open-settings"`;
  - differs from `EVENT_TOGGLE_SIDEBAR` and `EVENT_TOGGLE_COMMIT_RAIL`;
  - is absent from the cache-event names.

  Mirror it as `EVENT_OPEN_SETTINGS` in `src/types.ts` beside `EVENT_TOGGLE_SIDEBAR` (`application-menu`: *Settings Menu Item*).
- [x] 1.2 In `crates/specforge/src/menu.rs`, build the app submenu as `.about(…)`, `.separator()`, the Settings item, `.separator()`, `.services()`, followed by the existing items. The Settings item is `MenuItem::with_id(handle, EVENT_OPEN_SETTINGS, "Settings…", true, Some("Cmd+,"))`. Widen `handle_menu_event`'s id guard to accept `EVENT_OPEN_SETTINGS`, so it shows and focuses the main window before emitting, exactly as the pane toggles do. Update the module and function doc comments that list what the menu emits (`application-menu`: *Settings Menu Item*).
- [x] 1.3 In `src/api.ts`, add `onOpenSettings(handler)`, a `listenLogged` on `EVENT_OPEN_SETTINGS`, declared the way the pane-toggle listeners are. The event is Tauri-only, so in the browser the listener simply never fires.

## 2. Pure modules: groups and fields

- [x] 2.1 Create `src/settingsGroups.ts`, a pure module (no React, no API calls) in the style of `docWidth.ts`. It exports:
  - `SETTINGS_GROUPS = ["workspaces", "layout", "integrations", "identity", "desktop"] as const` and the `SettingsGroup` type;
  - `SETTINGS_GROUP_LABELS`: Workspaces, Layout, Integrations, Identity, Desktop app;
  - `DEFAULT_SETTINGS_GROUP = "workspaces"`;
  - `isSettingsGroup(value)`;
  - `groupsForHost({ desktop })`: the offered groups in order, without `desktop` when `desktop` is false;
  - `effectiveSettingsGroup(group, host)`: the group if the host offers it, else the default.

  (`settings-view`: *Settings Are Organised Into Groups*, *Groups With Nothing to Offer Are Omitted*)
- [x] 2.2 Add `src/settingsGroups.test.ts` covering:
  - the order and the labels;
  - `isSettingsGroup` for each id, for `""`, for `"Workspaces"` (case matters) and for an unknown slug;
  - `groupsForHost` on the desktop (five groups) and in the browser (four, no `desktop`);
  - `effectiveSettingsGroup` falling back only for `desktop` on the browser host.
- [x] 2.3 Create `src/settingsFields.ts`, exporting:
  - `parsePort(raw)`: digits only after trimming, as an integer $$1 \le p \le 65535$$, otherwise an error message naming the range;
  - `parsePollSeconds(raw)`: digits only, an integer of at least 1;
  - `parseText(raw)`: trimmed, where empty means `null`;
  - `commitDecision(draft, stored, parse)`, returning `{ kind: "unchanged" } | { kind: "invalid"; message } | { kind: "write"; value }`. It compares the parsed value with the stored one, so `" 4317 "` against a stored 4317 is unchanged.

  Add `src/settingsFields.test.ts` covering:
  - the boundaries 0, 1, 65535 and 65536;
  - `"43.5"`, `"4e3"`, `"abc"` and `""`;
  - surrounding whitespace;
  - every `commitDecision` branch.

  (`settings-view`: *Settings Persist by One Rule*)

## 3. Routing: the settings address carries its group

- [x] 3.1 In `src/routing/address.ts`, change the settings variant to `{ kind: "settings"; group: SettingsGroup }`, importing the type from `src/settingsGroups.ts`. Update the module header's list of addressable views (`view-routing`: *Addressable Viewing State*).
- [x] 3.2 In `src/routing/codec.ts`, encode a settings address as `/settings/<group>`, and decode settings paths as follows:
  - the bare `/settings` decodes to the Workspaces group;
  - `/settings/<id>` decodes to that group when `isSettingsGroup(id)`;
  - every other settings path is unresolvable, whether an unknown id or a deeper path such as `/settings/layout/x`.

  Update the grammar comment at the top of the file. In `codec.test.ts`:
  - add the round trip of all five groups;
  - add `/settings` decoding to Workspaces;
  - add an unknown group and a deeper path, each decoding to unresolvable;
  - update every existing settings expectation to carry `group`.

  (`view-routing`: *Address and URL Round-Trip Through a Pure Codec*)
- [x] 3.3 In `src/routing/resolve.ts`, make the settings resolution carry the address's group: replace the `RESOLVED_SETTINGS` constant with a value built from the address, and update `resolve.test.ts`. Confirm that `nodeId.ts` (and its test) still maps a settings address of any group to `null`, and that nothing in `history.test.ts` assumed a group-less settings address.

## 4. Settings primitives

- [x] 4.1 Create `src/components/settings/Switch.tsx`, rendering `<input type="checkbox" role="switch">` and taking `checked`, `onChange`, `disabled`, `id` and an optional `aria-label`. In `src/App.css`, add `.settings-switch` with:
  - `appearance: none`, a 34×20 fully rounded track and a 14px `::before` knob;
  - **off:** a `--surface-2` track, a `--border-strong` edge and a `--text-faint` knob at the leading end;
  - **on:** an `--accent` edge and knob over `--surface`, with the knob at the trailing end and no accent fill or glow;
  - `:focus-visible` using the shared focus recipe;
  - no knob transition under `prefers-reduced-motion`;
  - under `@media (pointer: coarse)`, an `::after` hit area bounded by its row.

  (`visual-identity`: *Settings Switch Control*, *Accent Color*)
- [x] 4.2 Create `src/components/settings/SettingsRow.tsx`. It renders:
  - a `<label htmlFor>` title and an optional description at the leading edge;
  - the control at the trailing edge;
  - an optional inline error (`role="alert"`).

  In `src/App.css`, add `.settings-row` with `--space-3` vertical padding and the existing row dividers. Add a container query that stacks the control below the text when the row is too narrow for both, and delete `.settings-toggle-row` (`settings-view`: *Settings Rows*; `visual-identity`: *List-Row Vertical Rhythm Tuned for 4K @ 100%*).
- [x] 4.3 Create `src/components/settings/useSettingSwitch.ts`, an optimistic flip that takes the loaded value and the setter. On failure it reverts and exposes a row-scoped message (via `prettifyError`) instead of calling `console.warn`. It replaces every hand-rolled `handle…Toggle` in today's view (`settings-view`: *Settings Persist by One Rule*).
- [x] 4.4 Create `src/components/settings/CommittedField.tsx`, a text input that commits on Enter or blur through `commitDecision`. Its edge cases:
  - **Escape:** stop propagation, so `App`'s window-level Escape fallback does not close Settings. Restore the stored value, and set an `abandoning` ref so the blur that follows does not commit, as `WorkspaceRow` does today.
  - **Invalid value:** show the parser's message inline.
  - **Rejected write:** show the error inline and reset the draft to the stored value.
  - **Number fields:** use `inputMode="numeric"` text inputs, never `type="number"`.

  (`settings-view`: *Settings Persist by One Rule*)
- [x] 4.5 Create `src/components/settings/CredentialForm.tsx`: password-type fields and an explicit Save. It writes only on Save, or on Enter in the token field as today, then re-reads the configuration and reports either "Saved…" or the error. Create `src/components/settings/IntegrationCard.tsx`. Its header row holds the name, description and `Switch`. While off, it adds a stored-credential line with Remove. While on, it renders its children:
  - the credential form;
  - a `<details>` with the instructions, open by default only while no credential is stored;
  - the slot line.

  (`settings-view`: *Opt-In Integrations Collapse While Off*)
- [x] 4.6 Create `src/components/settings/useProviderConfig.ts` with `useBitbucketConfig()` and `useGithubConfig()`. Each fetches on mount, exposes a load failure, and adopts `onPullRequestPanelMoved` for its own provider, lifting the effect today's two sections duplicate. Move `PANEL_POSITIONS` (slot values and labels) into a module that both the Layout group and the cards import (`settings-view`: *The Layout Group Gathers the Side Panes' Occupants*).

## 5. The five groups and the view shell

- [x] 5.1 Create `src/components/settings/WorkspacesGroup.tsx` and move into it, from `SettingsView.tsx`:
  - the workspaces list and `WorkspaceRow`;
  - the add-workspace control (a dialog on the desktop, a path field in the browser).

  Replace `WorkspaceRow`'s `<button role="switch" className="workspace-toggle">` with `Switch`, keeping its `aria-label` (with the shared-scope suffix), its `title` and its row-scoped error. Add the WSL poll interval as a `CommittedField` using `parsePollSeconds`, rendered only when its getter returns a number (`workspace-registry`: *Settings View*; `settings-view`: *Groups With Nothing to Offer Are Omitted*).
- [x] 5.2 Create `src/components/settings/LayoutGroup.tsx`, holding:
  - **Reading width:** the choice row and its sample, moved from `ReadingWidthSection` together with its comments.
  - **Commit history:** the switch row. Shorten the sentence that explains the rail coupling, since the panel rows now sit beneath it.
  - **Panel positions:** one radio row per provider, BitBucket then GitHub. While a provider is off, its row says so and adds a "Turn on in Integrations" button that opens the Integrations group, and the choice stays operable.

  (`settings-view`: *The Layout Group Gathers the Side Panes' Occupants*)
- [x] 5.3 Create `src/components/settings/IntegrationsGroup.tsx`: four `IntegrationCard`s, pull requests first (GitHub, then BitBucket), then usage (Claude, then ChatGPT).
  - **GitHub card:** the token `CredentialForm`.
  - **BitBucket card:** the username and token form.
  - **Instructions:** keep both cards' instructions verbatim from today: scopes, fine-grained versus classic, SSO, `gh auth token`, environment-variable precedence, and the `SettingsUrl` rendering.
  - **Removal while off:** clears only the token, with `setGithubToken("")` and `setBitbucketCredentials(username ?? "", "")`.
  - **Slot line:** an enabled pull-request card's slot line names the panel's slot and adds a "Change in Layout" button.
  - **Usage cards:** the header row only.

  (`settings-view`: *Opt-In Integrations Collapse While Off*; `bitbucket-pull-requests`: *Credentials Are Stored Write-Only*; `github-pull-requests`: *The GitHub Token Is Stored Write-Only*)
- [x] 5.4 Create `src/components/settings/IdentityGroup.tsx`. Move `IdentitySection` and its helpers (`authorKey`, `authorLabel`, `makeAuthor`, `AddIdentityForm`) into it, and make the display name a `CommittedField`, so Escape abandons the edit.
- [x] 5.5 Create `src/components/settings/DesktopGroup.tsx`, rendered only when `isTauri()`, holding:
  - the Launch at login and Notifications switch rows;
  - web access, from `WebServerSection`;
  - a port `CommittedField` using `parsePort`, replacing the per-keystroke `changePort`;
  - the "restart to apply" note;
  - the Tailscale switch and its two `CommittedField`s (name override and allowed logins);
  - the SSH/Tailscale walkthrough, moved under a `<details>`.

  (`workspace-registry`: *Settings View*; `web-ui`: *Desktop-Only Settings Are Hidden in the Web UI*)
- [x] 5.6 Create `src/components/settings/SettingsNav.tsx`, which renders both navigation forms from `groupsForHost`, each calling `onSelectGroup`:
  - a list of buttons, with `aria-current="page"` on the shown group;
  - a native `<select>`.

  Create `src/components/settings/SettingsView.tsx`, the shell:
  - the header (title and close button);
  - `SettingsNav`;
  - the group heading;
  - the component for the `group` prop;
  - the App-owned reading width and Commit history passed through to `LayoutGroup`.

  In `src/App.css`, make `.settings-view` a query container (`container-type: inline-size`):
  - the narrow layout is the default stylesheet: select shown, list hidden;
  - `@container (min-width: 640px)` switches to the list beside the content;
  - the content column is capped at 720px.

  Delete `src/components/SettingsView.tsx` once nothing imports it (`settings-view`: *Group Navigation Follows the View's Own Width*, *Settings Rows*).

## 6. App wiring

- [x] 6.1 In `src/App.tsx`, import `SettingsView` from `./components/settings/SettingsView`. Pass it the resolved `group` and `onSelectGroup={(g) => go({ kind: "settings", group: g })}`. A group switch replaces the history entry through `go()`'s existing overlay rule. Pass no `replace` option: the omission is deliberate, and a one-line comment should say so (`view-routing`: *History Entry Discipline*).
- [x] 6.2 In `src/App.tsx`, make every navigation into Settings name its group:
  - the sidebar Settings row opens `workspaces`;
  - `handleOpenShip`, for an entry that cannot be opened, opens `workspaces`;
  - `DisabledAddressNotice`'s `onOpenSettings` opens `workspaces`.

  (`settings-view`: *Entry Points Open the Workspaces Group*; `dashboard`: *Ship Selection Opens the Archive Browser*; `view-routing`: *Cold-Load Address Resolution*)
- [x] 6.3 In `src/App.tsx`, add an effect: when the resolved settings group is not offered on this host (`effectiveSettingsGroup` differs from it), call `go({ kind: "settings", group: "workspaces" }, { replace: true })` (`settings-view`: *Groups With Nothing to Offer Are Omitted*).
- [x] 6.4 In `src/App.tsx`, subscribe to `onOpenSettings`. When the current address is not a settings address, call `go({ kind: "settings", group: "workspaces" })`; otherwise do nothing, so the shown group stays. Add no keydown handler for ⌘, (`application-menu`: *Settings Menu Item*).
- [x] 6.5 Remove the CSS only the old view used: `.settings-toggle-row`, `.workspace-toggle`, `.workspace-toggle-knob` and their reduced-motion block. Grep `src/` to confirm nothing still references them.

## 7. Documentation

- [x] 7.1 Add a short *Settings* paragraph to `src/CLAUDE.md` saying four things:
  - there are five groups, and `src/settingsGroups.ts` is the membership source;
  - adding a setting means picking its group and using `SettingsRow`, `Switch`, `CommittedField` or `CredentialForm`, never adding a new free-standing section;
  - the save rule;
  - a desktop-only setting belongs in Desktop app.

## 8. Verification

- [x] 8.1 Run `bun install && bun run build` once in the fresh worktree. Then confirm `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test` are green.
- [x] 8.2 Confirm `bun run build` and `bun test` are green. Root `bun test` discovery grows by `settingsGroups.test.ts` and `settingsFields.test.ts`. That is expected, and is not the `bunfig.toml` trap.
- [x] 8.3 Mutation-test the diff: `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Only `events.rs` changes in a gated crate (one constant and one test), so expect no mutants in scope.
- [x] 8.4 Smoke-test in the browser loop. Run a debug `specforge-serve` on isolated state, serving the rebuilt `dist/`, and drive it with Chrome MCP. Walk the scenarios at Settings view widths of 320, 680 and 1,600px, setting the width by sizing `.app-shell` or the panes.
  - **Groups and navigation:**
    - exactly one group shows;
    - below 640px there is a select, and at 640px and above a list;
    - only one navigation form is exposed to assistive technology.
  - **Addresses and history:**
    - `/settings` opens Workspaces;
    - `/settings/desktop` falls back to Workspaces with a replace;
    - `/settings/bogus` reports not found;
    - Back after visiting three groups closes Settings.
  - **Workspaces:** both the Dashboard entry for a parked workspace and the disabled notice land on Workspaces.
  - **Integrations:**
    - an integration that is off is a single row;
    - turning GitHub on shows its token form with the instructions expanded;
    - the instructions fold once a token is stored;
    - removing a token while the integration is off leaves it off.
  - **Layout:**
    - an off provider's panel-slot row shows the off note;
    - a slot changed in a second tab is reflected.
  - **Save rule:**
    - typing a port writes once, on Enter (check the invoke log);
    - 70000 is refused on the field;
    - Escape in the display name restores it and leaves Settings open;
    - a failed switch reports on its own row (make one `set_*` invoke reject by intercepting `fetch` in the page).
  - **Browser skin:** there is no Desktop app group.
- [x] 8.5 Smoke-test with `bun run wt:dev` on macOS:
  - SpecForge → Settings… sits between About and Services, and opens Workspaces from an artifact view.
  - Cmd+, with the main window hidden and a reader window focused shows the main window at Settings.
  - Cmd+, while Integrations is shown keeps Integrations.
  - One keypress makes one navigation.
  - Switches and rows render correctly in both the light and the dark scheme.
  - Space flips a focused switch.
