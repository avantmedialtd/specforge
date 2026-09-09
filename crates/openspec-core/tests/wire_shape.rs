//! The flat-workspace row's wire shape.
//!
//! `WorkspaceView` is a *tagged enum*, and `#[serde(rename_all = "camelCase")]`
//! on an enum renames its **variants**, not the fields inside a struct variant.
//! Without `rename_all_fields`, `Flat`'s `display_name` therefore went out as
//! `display_name` while `src/types.ts` declares `displayName`, so every
//! `view.displayName ?? view.workspace.name` read was `undefined` and the
//! basename fallback silently won. The tint set in the same dialog *did* apply,
//! because `color` is a single word and spelled identically either way — so the
//! user watched half of their edit take effect.
//!
//! This asserts the emitted key set directly, because the fix is a serde
//! attribute and an attribute is not a mutable line: `cargo mutants` cannot
//! cover it, so this test is the only thing that does. It lives in
//! `openspec-core` rather than beside the cross-crate guard in `openspec-app`
//! because `.cargo/mutants.toml` sets `test_workspace = false` — only the
//! owning package's tests run for a mutant in this crate, and
//! `cargo test -p openspec-core` alone must fail if the attribute is dropped.
//!
//! The general "no snake_case key at any depth" contract, over every type the
//! frontend reads, is `openspec-app/tests/wire_shape.rs`.

use openspec_core::repo_view::WorkspaceView;
use openspec_core::types::{PaletteColor, WorkspaceFolder};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn flat_fixture() -> WorkspaceView {
    WorkspaceView::Flat {
        workspace: WorkspaceFolder {
            uri: PathBuf::from("/tmp/notes"),
            name: "notes".to_string(),
        },
        changes: Vec::new(),
        display_name: Some("Nice Name".to_string()),
        color: Some(PaletteColor::Indigo),
        disabled: false,
    }
}

/// The exact top-level key set of a serialized `WorkspaceView::Flat`.
///
/// Asserted as a whole rather than as "contains `displayName`" so that it fails
/// in both directions: dropping `rename_all_fields` reintroduces
/// `display_name`, and dropping `tag`/`rename_all` loses the `kind`
/// discriminant the TypeScript union matches on. `disabled` is
/// `skip_serializing` and must never appear.
#[test]
fn flat_variant_emits_exactly_the_keys_the_frontend_declares() {
    let value = serde_json::to_value(flat_fixture()).expect("serialize");
    let keys: BTreeSet<&str> = value
        .as_object()
        .expect("flat variant serializes as a JSON object")
        .keys()
        .map(String::as_str)
        .collect();

    let expected: BTreeSet<&str> = ["kind", "workspace", "changes", "displayName", "color"]
        .into_iter()
        .collect();

    assert_eq!(
        keys, expected,
        "WorkspaceView::Flat's emitted keys drifted from what src/types.ts declares"
    );
}

/// The value has to survive too, not just the key: a `displayName` key holding
/// the wrong thing would pass the key-set assertion above.
#[test]
fn flat_variant_carries_the_display_name_override() {
    let value = serde_json::to_value(flat_fixture()).expect("serialize");

    assert_eq!(value["kind"], "flat");
    assert_eq!(value["displayName"], "Nice Name");
    assert!(
        value.get("display_name").is_none(),
        "snake_case display_name is still on the wire: `rename_all` on an enum \
         renames variants, so the struct variant needs `rename_all_fields`"
    );
}

/// The `Repo` variant is a newtype whose inner `RepoView` carries its own
/// `rename_all`, so the enum-wide attribute must not disturb it.
#[test]
fn repo_variant_still_flattens_into_the_inner_view() {
    let view = WorkspaceView::Repo(openspec_core::repo_view::RepoView {
        repo_id: PathBuf::from("/tmp/repo/.git"),
        main_worktree: PathBuf::from("/tmp/repo"),
        name: "repo".to_string(),
        default_branch: Some("master".to_string()),
        active: Vec::new(),
        archived: Vec::new(),
        display_name: Some("Repo Name".to_string()),
        color: Some(PaletteColor::Teal),
        dirty: true,
        dirty_worktrees: vec![PathBuf::from("/tmp/repo")],
        has_uncommitted_specs: true,
        disabled: false,
    });

    let value = serde_json::to_value(view).expect("serialize");
    assert_eq!(value["kind"], "repo");
    assert_eq!(value["displayName"], "Repo Name");
    assert_eq!(value["mainWorktree"], "/tmp/repo");
    assert_eq!(value["hasUncommittedSpecs"], true);
}
