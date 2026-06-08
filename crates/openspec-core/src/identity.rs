//! Developer identity: who the local developer is, and which observed git
//! identities fold onto them.
//!
//! Pure and Tauri-free so it is unit-testable from `cargo test`. The Tauri
//! shell persists the user's chosen [`IdentityConfig`] alongside app settings
//! and reads the local git identity via [`crate::git::git_identity`]; this
//! module only models identities and answers "is this observed author me?".
//!
//! The key design point (see the `developer-identity` capability): an
//! [`Achievement`](crate::activity_log::Achievement) stores the **raw** author
//! it was observed with, and `is_me` resolution runs against the *current*
//! config at query time. Adding an alias therefore retroactively reclaims past
//! activity without rewriting the append-only log.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// A raw developer identity as observed from git. Either component MAY be
/// absent — a repository may set only `user.name`, and a name-only commit
/// trailer carries no email. An [`Author`] with both fields empty is treated
/// as "no identity" by [`normalized_key`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl Author {
    pub fn new(name: Option<String>, email: Option<String>) -> Self {
        Self { name, email }
    }

    /// A human-facing label: the name if present, else the email, else "Unknown".
    pub fn display(&self) -> String {
        self.name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(self.email.as_deref().filter(|s| !s.trim().is_empty()))
            .unwrap_or("Unknown")
            .to_string()
    }
}

/// The developer's identity configuration, persisted in app data. `aliases`
/// holds **every** identity that resolves to the canonical developer ("me") —
/// the local git identity plus any folded-in variants. The first entry is
/// treated as primary (the source of the profile avatar). `display_name` is the
/// canonical label shown on the profile, independent of any git name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityConfig {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub aliases: Vec<Author>,
}

impl IdentityConfig {
    /// The primary identity (avatar / canonical-key source): the first alias.
    pub fn primary(&self) -> Option<&Author> {
        self.aliases.first()
    }

    /// The canonical normalised key for the developer (the primary identity's
    /// key), or `None` when no identity is configured.
    pub fn primary_key(&self) -> Option<String> {
        self.primary().and_then(normalized_key)
    }

    /// The display label: the configured `display_name`, falling back to the
    /// primary identity's display, then "You".
    pub fn label(&self) -> String {
        self.display_name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .or_else(|| self.primary().map(Author::display))
            .unwrap_or_else(|| "You".to_string())
    }
}

/// A named person on the contributor roster: a custom display name plus the set
/// of git identities that all fold onto them. The per-author leaderboard
/// collapses an observed author into the person holding its identity and labels
/// the row with [`Person::label`]. This is the multi-person generalization of
/// [`IdentityConfig`], which remains the distinguished canonical developer
/// ("me"); see the `developer-identity` capability. Presentation only — the
/// roster never feeds the deterministic season generators, and "me" always wins
/// over a roster person at resolution time (see [`roster_index`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub identities: Vec<Author>,
}

impl Person {
    /// The person's canonical attribution key: the first identity that yields a
    /// usable normalised key, or `None` when the person has no usable identity.
    /// Mirrors [`IdentityConfig::primary_key`].
    pub fn primary_key(&self) -> Option<String> {
        self.identities.iter().find_map(normalized_key)
    }

    /// The display label: the custom `display_name`, falling back to the first
    /// identity's [`Author::display`], then "Unknown".
    pub fn label(&self) -> String {
        self.display_name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .or_else(|| self.identities.first().map(Author::display))
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

/// The normalised attribution key for an identity: the lowercased, trimmed
/// email when present and non-empty, otherwise the lowercased, trimmed name.
/// `None` when neither yields a non-empty value. Email is the strong signal, so
/// an email-bearing identity keys on its email and a name-only identity keys on
/// its name.
pub fn normalized_key(author: &Author) -> Option<String> {
    if let Some(email) = author.email.as_deref() {
        let e = email.trim().to_lowercase();
        if !e.is_empty() {
            return Some(e);
        }
    }
    if let Some(name) = author.name.as_deref() {
        let n = name.trim().to_lowercase();
        if !n.is_empty() {
            return Some(n);
        }
    }
    None
}

/// Whether an observed `author` resolves to the canonical developer under
/// `config`: its normalised key matches the key of any configured alias.
/// Because email-bearing authors key on email and name-only authors key on
/// name, an email author matches only a same-email alias and a name-only
/// author matches only a name-only alias — exactly the spec's rule.
pub fn is_me(author: &Author, config: &IdentityConfig) -> bool {
    let Some(key) = normalized_key(author) else {
        return false;
    };
    config
        .aliases
        .iter()
        .filter_map(normalized_key)
        .any(|k| k == key)
}

/// The distinct git identities observed across `paths` (registered workspace
/// folders), in first-seen order, deduped by normalised key. Used to seed the
/// identity config on first run and to suggest aliases in Settings. Paths with
/// no resolvable git identity contribute nothing.
pub fn detect_candidate_identities(paths: &[PathBuf]) -> Vec<Author> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        if let Some(author) = crate::git::git_identity(path) {
            if let Some(key) = normalized_key(&author) {
                if seen.insert(key) {
                    out.push(author);
                }
            }
        }
    }
    out
}

/// Build a lookup from every rostered identity's normalised key to its person's
/// `(canonical_key, label)`, so an observed author can be resolved to its named
/// person in one map hit. Identities without a usable key are skipped, as are
/// people with no usable identity. When a key appears under more than one person
/// the first in roster order wins — but the editing layer enforces
/// single-assignment (see [`assign_identity`]), so that is only a defensive
/// tie-break. Resolution callers MUST check [`is_me`] *before* this map, so an
/// identity that is also the developer's resolves to "me", not to a roster
/// person (you-precedence).
pub fn roster_index(people: &[Person]) -> HashMap<String, (String, String)> {
    let mut map: HashMap<String, (String, String)> = HashMap::new();
    for person in people {
        let Some(canonical) = person.primary_key() else {
            continue;
        };
        let label = person.label();
        for identity in &person.identities {
            if let Some(key) = normalized_key(identity) {
                map.entry(key)
                    .or_insert_with(|| (canonical.clone(), label.clone()));
            }
        }
    }
    map
}

/// Record `author` as an identity of the person at `target`, enforcing
/// single-assignment: the identity's normalised key is first removed from every
/// *other* person, then appended to the target (deduped, so a re-add is a
/// no-op). An `author` with no usable key, or an out-of-range `target`, leaves
/// the roster unchanged. You-precedence (an identity that is also "me") is
/// resolved at query time, not here, so this never consults the developer config.
pub fn assign_identity(people: &mut [Person], target: usize, author: Author) {
    let Some(key) = normalized_key(&author) else {
        return;
    };
    if target >= people.len() {
        return;
    }
    for (i, person) in people.iter_mut().enumerate() {
        if i != target {
            person
                .identities
                .retain(|a| normalized_key(a).as_deref() != Some(key.as_str()));
        }
    }
    let person = &mut people[target];
    if !person
        .identities
        .iter()
        .any(|a| normalized_key(a).as_deref() == Some(key.as_str()))
    {
        person.identities.push(author);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn author(name: Option<&str>, email: Option<&str>) -> Author {
        Author::new(name.map(str::to_string), email.map(str::to_string))
    }

    #[test]
    fn normalized_key_prefers_email_lowercased() {
        let a = author(Some("István Antal"), Some("Istvan@Example.UK "));
        assert_eq!(normalized_key(&a).as_deref(), Some("istvan@example.uk"));
    }

    #[test]
    fn normalized_key_falls_back_to_name() {
        let a = author(Some("  Istvan  "), None);
        assert_eq!(normalized_key(&a).as_deref(), Some("istvan"));
    }

    #[test]
    fn normalized_key_is_none_for_empty_identity() {
        assert_eq!(normalized_key(&author(None, None)), None);
        assert_eq!(normalized_key(&author(Some("  "), Some(""))), None);
    }

    #[test]
    fn is_me_matches_on_email() {
        let config = IdentityConfig {
            display_name: Some("Me".into()),
            aliases: vec![author(Some("Me"), Some("me@example.com"))],
        };
        // Same email, different name → still me.
        assert!(is_me(
            &author(Some("M. E."), Some("ME@example.com")),
            &config
        ));
        // Different email → not me.
        assert!(!is_me(
            &author(Some("Me"), Some("other@example.com")),
            &config
        ));
    }

    #[test]
    fn is_me_matches_name_only_alias() {
        let config = IdentityConfig {
            display_name: None,
            aliases: vec![author(Some("istvan"), None)],
        };
        assert!(is_me(&author(Some("Istvan"), None), &config));
        // An email-bearing author keys on email, so it does NOT match a
        // name-only alias even when the names coincide.
        assert!(!is_me(&author(Some("istvan"), Some("x@y.z")), &config));
    }

    #[test]
    fn is_me_across_multiple_aliases() {
        let config = IdentityConfig {
            display_name: Some("Me".into()),
            aliases: vec![
                author(Some("Me"), Some("work@corp.com")),
                author(Some("Me"), Some("personal@home.net")),
            ],
        };
        assert!(is_me(&author(None, Some("personal@home.net")), &config));
        assert!(is_me(&author(None, Some("work@corp.com")), &config));
        assert!(!is_me(&author(None, Some("nope@x.com")), &config));
    }

    #[test]
    fn is_me_false_for_empty_author_or_config() {
        let empty = IdentityConfig::default();
        assert!(!is_me(&author(Some("x"), None), &empty));
        let config = IdentityConfig {
            display_name: None,
            aliases: vec![author(Some("x"), None)],
        };
        assert!(!is_me(&author(None, None), &config));
    }

    #[test]
    fn label_prefers_display_name_then_primary_then_you() {
        let with_name = IdentityConfig {
            display_name: Some("István".into()),
            aliases: vec![author(Some("ia"), Some("a@b.c"))],
        };
        assert_eq!(with_name.label(), "István");

        let no_name = IdentityConfig {
            display_name: None,
            aliases: vec![author(Some("ia"), Some("a@b.c"))],
        };
        assert_eq!(no_name.label(), "ia");

        assert_eq!(IdentityConfig::default().label(), "You");
    }

    fn person(name: Option<&str>, ids: &[(Option<&str>, Option<&str>)]) -> Person {
        Person {
            display_name: name.map(str::to_string),
            identities: ids.iter().map(|(n, e)| author(*n, *e)).collect(),
        }
    }

    #[test]
    fn person_primary_key_and_label() {
        let p = person(
            Some("Jane"),
            &[
                (Some("Jane"), Some("Jane@Corp.com")),
                (None, Some("jdoe@corp.com")),
            ],
        );
        // Primary key is the first usable identity's normalised (email) key.
        assert_eq!(p.primary_key().as_deref(), Some("jane@corp.com"));
        // Label prefers the custom display name.
        assert_eq!(p.label(), "Jane");
        // Without a custom name, the label falls back to the first identity.
        let unnamed = person(None, &[(Some("jdoe"), None)]);
        assert_eq!(unnamed.label(), "jdoe");
        // A person with no usable identity has no key and an "Unknown" label.
        let empty = person(None, &[]);
        assert_eq!(empty.primary_key(), None);
        assert_eq!(empty.label(), "Unknown");
    }

    #[test]
    fn roster_index_folds_identities_to_one_person() {
        let people = vec![person(
            Some("Jane"),
            &[
                (Some("Jane"), Some("jane@corp.com")),
                (None, Some("jdoe@corp.com")),
            ],
        )];
        let idx = roster_index(&people);
        // Both of Jane's identities map to the same canonical key + label.
        let jane = ("jane@corp.com".to_string(), "Jane".to_string());
        assert_eq!(idx.get("jane@corp.com"), Some(&jane));
        assert_eq!(idx.get("jdoe@corp.com"), Some(&jane));
        // An unrostered identity is absent.
        assert_eq!(idx.get("someone@else.com"), None);
    }

    #[test]
    fn roster_index_skips_keyless_identities_and_people() {
        let people = vec![
            person(Some("Ghost"), &[(None, None)]),
            person(Some("Real"), &[(Some("Real"), Some("real@x.io"))]),
        ];
        let idx = roster_index(&people);
        assert_eq!(idx.len(), 1);
        assert!(idx.contains_key("real@x.io"));
    }

    #[test]
    fn assign_identity_is_exclusive_across_people() {
        let mut people = vec![
            person(Some("Jane"), &[(Some("Jane"), Some("jane@corp.com"))]),
            person(Some("Bot"), &[(None, Some("ci@github.com"))]),
        ];
        // Reassign jane@corp.com (currently Jane's) onto Bot.
        assign_identity(&mut people, 1, author(None, Some("jane@corp.com")));
        // Removed from Jane…
        assert!(people[0]
            .identities
            .iter()
            .all(|a| normalized_key(a).as_deref() != Some("jane@corp.com")));
        // …and now held by Bot, exactly once.
        let on_bot = people[1]
            .identities
            .iter()
            .filter(|a| normalized_key(a).as_deref() == Some("jane@corp.com"))
            .count();
        assert_eq!(on_bot, 1);
        // Re-adding the same identity to Bot is a no-op (still once).
        assign_identity(&mut people, 1, author(Some("J"), Some("JANE@corp.com")));
        let on_bot_again = people[1]
            .identities
            .iter()
            .filter(|a| normalized_key(a).as_deref() == Some("jane@corp.com"))
            .count();
        assert_eq!(on_bot_again, 1);
    }

    #[test]
    fn assign_identity_ignores_empty_author_and_bad_target() {
        let mut people = vec![person(
            Some("Jane"),
            &[(Some("Jane"), Some("jane@corp.com"))],
        )];
        assign_identity(&mut people, 0, author(None, None)); // no usable key
        assign_identity(&mut people, 9, author(None, Some("x@y.z"))); // out of range
        assert_eq!(people[0].identities.len(), 1);
    }
}
