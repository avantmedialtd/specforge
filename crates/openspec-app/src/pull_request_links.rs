//! Links between open pull requests and the git worktrees SpecForge tracks
//! (the `pull-request-worktree-links` capability).
//!
//! A local, read-only join: the pull-request snapshots both pollers already
//! hold, against each tracked repository's worktree branches and upstreams
//! (`openspec_core::repo_view::WorktreeRef`) and its remotes
//! (`WatcherManager::remotes`). No network request and no credential — the
//! join reads only data SpecForge already has.
//!
//! Everything here is pure and tested; `AppService::pull_request_links`
//! gathers the inputs and calls [`link_pull_requests`]
//! (`link-pull-requests-to-worktrees` design D1, D6, D7).

use std::path::{Path, PathBuf};

use openspec_core::repo_view::WorktreeRef;
use openspec_core::{parse_remote_url, Remote, RemoteIdentity, RemoteTransport};
use serde::Serialize;

use crate::events::PullRequestProvider;
use crate::pull_requests::{ChecksState, PullRequestSummary, ReviewSummary};

// ---- the wire shape (design D7) ----

/// Whether a linked pull request is the viewer's own or awaits their review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PullRequestRole {
    Authored,
    ReviewRequested,
}

/// What the change header's chip and the switcher's marker need of a linked
/// pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedPullRequest {
    pub provider: PullRequestProvider,
    pub role: PullRequestRole,
    pub id: u64,
    pub title: String,
    pub url: String,
    pub repo_full_name: String,
    pub draft: bool,
    pub checks: Option<ChecksState>,
    pub conflicting: bool,
    pub review: Option<ReviewSummary>,
}

/// A worktree and the pull requests linked to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreePullRequests {
    pub worktree_path: PathBuf,
    pub pull_requests: Vec<LinkedPullRequest>,
}

/// A worktree a pull request is linked to — enough for the frontend to find
/// the change hosted there, or the repository's file browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedWorktree {
    pub repo_id: PathBuf,
    pub worktree_path: PathBuf,
    pub branch: Option<String>,
}

/// A pull request, keyed by its web URL, and the worktrees linked to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestWorktrees {
    pub url: String,
    pub worktrees: Vec<LinkedWorktree>,
}

/// The links snapshot `get_pull_request_links` serves: the same links seen
/// from both ends (`pull-request-worktree-links`: *The Pull-Request Links
/// Snapshot*).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestLinks {
    pub worktrees: Vec<WorktreePullRequests>,
    pub pull_requests: Vec<PullRequestWorktrees>,
}

// ---- the inputs ----

/// One pull-request row with the provider and role it was listed under. The
/// caller passes rows in the order a worktree's list should show them:
/// BitBucket, then GitHub authored, then GitHub review-requested.
#[derive(Debug, Clone, Copy)]
pub struct PullRequestInput<'a> {
    pub provider: PullRequestProvider,
    pub role: PullRequestRole,
    pub row: &'a PullRequestSummary,
}

/// One tracked, warm repository: its identity, main worktree, every tracked
/// worktree's branch and upstream, and its remotes.
#[derive(Debug, Clone, Copy)]
pub struct RepoInput<'a> {
    pub repo_id: &'a Path,
    pub main_worktree: &'a Path,
    pub worktrees: &'a [WorktreeRef],
    pub remotes: &'a [Remote],
}

// ---- the rules ----

/// The hosts each provider serves its repositories from: the web host and its
/// documented SSH-over-HTTPS host.
fn provider_hosts(provider: PullRequestProvider) -> &'static [&'static str] {
    match provider {
        PullRequestProvider::Github => &["github.com", "ssh.github.com"],
        PullRequestProvider::Bitbucket => &["bitbucket.org", "altssh.bitbucket.org"],
    }
}

/// Every host SpecForge recognises; an SSH remote on any other host is taken
/// to be an SSH alias.
const KNOWN_HOSTS: [&str; 4] = [
    "github.com",
    "ssh.github.com",
    "bitbucket.org",
    "altssh.bitbucket.org",
];

/// Whether a remote identity agrees with a pull request's head repository:
/// the `owner/name` paths are equal ignoring case, and either the remote's
/// host is the provider's, or it is an SSH remote on an unrecognised host (an
/// alias) and the path alone decides. An HTTPS remote on another host — a
/// GitLab or enterprise remote — never agrees.
pub(crate) fn agrees(
    identity: &RemoteIdentity,
    provider: PullRequestProvider,
    head_repo: &str,
) -> bool {
    if !identity.path.eq_ignore_ascii_case(head_repo) {
        return false;
    }
    let host = identity.host.as_str();
    provider_hosts(provider).contains(&host)
        || (identity.transport == RemoteTransport::Ssh && !KNOWN_HOSTS.contains(&host))
}

/// Split a raw `<remote>/<branch>` upstream by the longest remote name that
/// prefixes it followed by `/` — remote names may contain `/`. `None` when no
/// remote prefixes it (a local upstream such as `main`, or a remote since
/// removed) or nothing follows the slash.
pub(crate) fn split_upstream<'a>(
    raw: &'a str,
    remote_names: &[&str],
) -> Option<(&'a str, &'a str)> {
    remote_names
        .iter()
        .filter(|name| {
            raw.len() > name.len() + 1
                && raw.starts_with(*name)
                && raw.as_bytes()[name.len()] == b'/'
        })
        .max_by_key(|name| name.len())
        .map(|name| (&raw[..name.len()], &raw[name.len() + 1..]))
}

/// Whether worktree `wt` is linked to pull request `pr` (design D6):
///
/// - **upstream rule** — the upstream's remote agrees with the head
///   repository, the upstream branch is the head branch, and either the
///   upstream has the worktree branch's own name or the pull request comes
///   from another repository (a fork, whose local branch may be named
///   differently);
/// - **branch-name rule** — the worktree's branch is the head branch, it has
///   no same-named upstream (which would be authoritative), and some remote of
///   the repository agrees with the head repository.
fn is_linked(
    branch: &str,
    upstream: Option<(&str, &str)>,
    remotes: &[(&str, Option<RemoteIdentity>)],
    provider: PullRequestProvider,
    pr: &PullRequestSummary,
) -> bool {
    let head_repo = pr.source_repo_full_name.as_str();
    let head_branch = pr.source_branch.as_str();
    let identity_of = |name: &str| {
        remotes
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, identity)| identity.as_ref())
    };
    let from_fork = !head_repo.eq_ignore_ascii_case(&pr.repo_full_name);

    if let Some((remote, upstream_branch)) = upstream {
        let upstream_rule = upstream_branch == head_branch
            && (upstream_branch == branch || from_fork)
            && identity_of(remote).is_some_and(|id| agrees(id, provider, head_repo));
        if upstream_rule {
            return true;
        }
        if upstream_branch == branch {
            // A same-named upstream says where this branch lives; it is
            // authoritative, and it did not match.
            return false;
        }
    }
    branch == head_branch
        && remotes
            .iter()
            .filter_map(|(_, identity)| identity.as_ref())
            .any(|id| agrees(id, provider, head_repo))
}

/// Join the pull requests with the repositories' worktrees
/// (`pull-request-worktree-links`: *Matching a Pull Request to a Worktree*).
///
/// Rows without a web URL or a head repository link nothing, nor do
/// worktrees without a branch. A pull request's worktrees are ordered main
/// worktrees first, then by path; a worktree's pull requests keep the input
/// order; worktrees are listed by path, pull requests in input order.
pub fn link_pull_requests(
    pull_requests: &[PullRequestInput<'_>],
    repos: &[RepoInput<'_>],
) -> PullRequestLinks {
    // (repo, worktree, parsed remotes, split upstream) for every candidate.
    struct Candidate<'a> {
        repo_id: &'a Path,
        is_main: bool,
        path: &'a Path,
        branch: &'a str,
        upstream: Option<(&'a str, &'a str)>,
        remotes: std::rc::Rc<Vec<(&'a str, Option<RemoteIdentity>)>>,
    }
    let mut candidates: Vec<Candidate<'_>> = Vec::new();
    for repo in repos {
        let names: Vec<&str> = repo.remotes.iter().map(|r| r.name.as_str()).collect();
        let remotes = std::rc::Rc::new(
            repo.remotes
                .iter()
                .map(|r| (r.name.as_str(), parse_remote_url(&r.url)))
                .collect::<Vec<_>>(),
        );
        for wt in repo.worktrees {
            let Some(branch) = wt.branch.as_deref() else {
                continue;
            };
            candidates.push(Candidate {
                repo_id: repo.repo_id,
                is_main: wt.path == repo.main_worktree,
                path: &wt.path,
                branch,
                upstream: wt
                    .upstream
                    .as_deref()
                    .and_then(|raw| split_upstream(raw, &names)),
                remotes: remotes.clone(),
            });
        }
    }
    candidates.sort_by(|a, b| (!a.is_main, a.path).cmp(&(!b.is_main, b.path)));

    let mut by_worktree: Vec<WorktreePullRequests> = Vec::new();
    let mut by_pull_request: Vec<PullRequestWorktrees> = Vec::new();
    for input in pull_requests {
        let pr = input.row;
        if pr.url.is_empty() || pr.source_repo_full_name.is_empty() {
            continue;
        }
        let linked: Vec<&Candidate<'_>> = candidates
            .iter()
            .filter(|c| is_linked(c.branch, c.upstream, &c.remotes, input.provider, pr))
            .collect();
        if linked.is_empty() {
            continue;
        }
        by_pull_request.push(PullRequestWorktrees {
            url: pr.url.clone(),
            worktrees: linked
                .iter()
                .map(|c| LinkedWorktree {
                    repo_id: c.repo_id.to_path_buf(),
                    worktree_path: c.path.to_path_buf(),
                    branch: Some(c.branch.to_string()),
                })
                .collect(),
        });
        let entry = LinkedPullRequest {
            provider: input.provider,
            role: input.role,
            id: pr.id,
            title: pr.title.clone(),
            url: pr.url.clone(),
            repo_full_name: pr.repo_full_name.clone(),
            draft: pr.draft,
            checks: pr.checks,
            conflicting: pr.conflicting,
            review: pr.review,
        };
        for c in linked {
            match by_worktree.iter_mut().find(|w| w.worktree_path == c.path) {
                Some(w) => w.pull_requests.push(entry.clone()),
                None => by_worktree.push(WorktreePullRequests {
                    worktree_path: c.path.to_path_buf(),
                    pull_requests: vec![entry.clone()],
                }),
            }
        }
    }
    by_worktree.sort_by(|a, b| a.worktree_path.cmp(&b.worktree_path));
    PullRequestLinks {
        worktrees: by_worktree,
        pull_requests: by_pull_request,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GH: PullRequestProvider = PullRequestProvider::Github;
    const BB: PullRequestProvider = PullRequestProvider::Bitbucket;

    fn pr(url: &str, base: &str, head_repo: &str, head_branch: &str) -> PullRequestSummary {
        PullRequestSummary {
            id: 7,
            title: "A change".to_string(),
            repo_full_name: base.to_string(),
            source_branch: head_branch.to_string(),
            destination_branch: "main".to_string(),
            url: url.to_string(),
            draft: false,
            updated_at_unix: 1_700_000_000,
            review: None,
            open_tasks: 0,
            author: None,
            checks: None,
            conflicting: false,
            unresolved_threads: 0,
            source_repo_full_name: head_repo.to_string(),
        }
    }

    fn remote(name: &str, url: &str) -> Remote {
        Remote {
            name: name.to_string(),
            url: url.to_string(),
        }
    }

    fn wt(path: &str, branch: Option<&str>, upstream: Option<&str>) -> WorktreeRef {
        WorktreeRef {
            path: PathBuf::from(path),
            branch: branch.map(str::to_string),
            upstream: upstream.map(str::to_string),
        }
    }

    /// Links for one repository at `/code/api` and one pull request.
    fn links_for(
        provider: PullRequestProvider,
        pr: &PullRequestSummary,
        remotes: &[Remote],
        worktrees: &[WorktreeRef],
    ) -> PullRequestLinks {
        let repo = RepoInput {
            repo_id: Path::new("/code/api/.git"),
            main_worktree: Path::new("/code/api"),
            worktrees,
            remotes,
        };
        link_pull_requests(
            &[PullRequestInput {
                provider,
                role: PullRequestRole::Authored,
                row: pr,
            }],
            &[repo],
        )
    }

    fn linked_paths(links: &PullRequestLinks) -> Vec<PathBuf> {
        links
            .pull_requests
            .iter()
            .flat_map(|p| p.worktrees.iter().map(|w| w.worktree_path.clone()))
            .collect()
    }

    const PR_URL: &str = "https://github.com/acme/api/pull/7";

    // ---------------------------------------------------------- split_upstream

    #[test]
    fn an_upstream_is_split_by_the_longest_remote_name() {
        assert_eq!(
            split_upstream("origin/mirror/feature", &["origin", "origin/mirror"]),
            Some(("origin/mirror", "feature"))
        );
        assert_eq!(
            split_upstream("origin/feature/x", &["origin"]),
            Some(("origin", "feature/x"))
        );
        assert_eq!(
            split_upstream("main", &["origin"]),
            None,
            "a local upstream"
        );
        assert_eq!(
            split_upstream("gone/feature", &["origin"]),
            None,
            "an unknown remote"
        );
        assert_eq!(
            split_upstream("origin/", &["origin"]),
            None,
            "nothing after the slash"
        );
        assert_eq!(
            split_upstream("originx/feature", &["origin"]),
            None,
            "a name prefix is not a remote"
        );
    }

    // ----------------------------------------------------------------- agrees

    fn id(transport: RemoteTransport, host: &str, path: &str) -> RemoteIdentity {
        RemoteIdentity {
            transport,
            host: host.to_string(),
            path: path.to_string(),
        }
    }

    #[test]
    fn known_hosts_must_be_the_providers() {
        use RemoteTransport::{Https, Ssh};
        assert!(agrees(&id(Ssh, "github.com", "acme/api"), GH, "Acme/API"));
        assert!(agrees(
            &id(Https, "ssh.github.com", "acme/api"),
            GH,
            "acme/api"
        ));
        assert!(agrees(
            &id(Https, "bitbucket.org", "acme/api"),
            BB,
            "acme/api"
        ));
        assert!(agrees(
            &id(Ssh, "altssh.bitbucket.org", "acme/api"),
            BB,
            "acme/api"
        ));
        assert!(!agrees(
            &id(Https, "bitbucket.org", "acme/api"),
            GH,
            "acme/api"
        ));
        assert!(!agrees(&id(Ssh, "github.com", "acme/api"), BB, "acme/api"));
        assert!(!agrees(
            &id(Ssh, "github.com", "acme/other"),
            GH,
            "acme/api"
        ));
    }

    #[test]
    fn an_ssh_alias_matches_on_the_path_and_an_unknown_https_host_never() {
        use RemoteTransport::{Https, Ssh};
        assert!(agrees(&id(Ssh, "github-work", "acme/api"), GH, "acme/api"));
        assert!(agrees(&id(Ssh, "github-work", "acme/api"), BB, "acme/api"));
        assert!(!agrees(
            &id(Https, "gitlab.com", "acme/api"),
            GH,
            "acme/api"
        ));
        assert!(!agrees(
            &id(Ssh, "github-work", "acme/other"),
            GH,
            "acme/api"
        ));
    }

    // ------------------------------------------------- the matching scenarios

    #[test]
    fn a_branch_pushed_with_tracking_links_through_its_upstream() {
        let pr = pr(PR_URL, "acme/api", "acme/api", "feature");
        let links = links_for(
            GH,
            &pr,
            &[remote("origin", "git@github.com:acme/api.git")],
            &[wt("/code/api", Some("feature"), Some("origin/feature"))],
        );
        assert_eq!(linked_paths(&links), vec![PathBuf::from("/code/api")]);
    }

    #[test]
    fn a_branch_pushed_without_tracking_links_by_name() {
        let pr = pr(PR_URL, "acme/api", "acme/api", "feature");
        let links = links_for(
            GH,
            &pr,
            &[remote("origin", "https://github.com/acme/api.git")],
            &[wt("/code/api", Some("feature"), Some("origin/main"))],
        );
        assert_eq!(linked_paths(&links), vec![PathBuf::from("/code/api")]);
        let untracked = links_for(
            GH,
            &pr,
            &[remote("origin", "https://github.com/acme/api.git")],
            &[wt("/code/api", Some("feature"), None)],
        );
        assert_eq!(linked_paths(&untracked), vec![PathBuf::from("/code/api")]);
    }

    #[test]
    fn a_worktree_branched_from_a_shared_branch_does_not_link_to_its_release() {
        // `feature` created from `origin/develop` tracks it; the open
        // release pull request's head is `develop` in the same repository.
        let release = pr(PR_URL, "acme/api", "acme/api", "develop");
        let links = links_for(
            GH,
            &release,
            &[remote("origin", "git@github.com:acme/api.git")],
            &[wt(
                "/code/api-feature",
                Some("feature"),
                Some("origin/develop"),
            )],
        );
        assert!(links.pull_requests.is_empty(), "{links:?}");
        // The develop worktree itself does link.
        let links = links_for(
            GH,
            &release,
            &[remote("origin", "git@github.com:acme/api.git")],
            &[wt("/code/api", Some("develop"), Some("origin/develop"))],
        );
        assert_eq!(linked_paths(&links), vec![PathBuf::from("/code/api")]);
    }

    #[test]
    fn a_renamed_local_branch_in_the_same_repository_does_not_link() {
        let pr = pr(PR_URL, "acme/api", "acme/api", "feature");
        let links = links_for(
            GH,
            &pr,
            &[remote("origin", "git@github.com:acme/api.git")],
            &[wt("/code/api", Some("wip"), Some("origin/feature"))],
        );
        assert!(links.pull_requests.is_empty(), "{links:?}");
    }

    #[test]
    fn a_forks_same_named_branch_does_not_link() {
        let from_fork = pr(PR_URL, "acme/api", "ada/api", "main");
        let links = links_for(
            GH,
            &from_fork,
            &[remote("origin", "git@github.com:acme/api.git")],
            &[wt("/code/api", Some("main"), Some("origin/main"))],
        );
        assert!(links.pull_requests.is_empty(), "{links:?}");
        // Even with the fork added as a remote, the same-named upstream is
        // authoritative: the maintainer's `main` is not the contributor's.
        let links = links_for(
            GH,
            &from_fork,
            &[
                remote("origin", "git@github.com:acme/api.git"),
                remote("ada", "git@github.com:ada/api.git"),
            ],
            &[wt("/code/api", Some("main"), Some("origin/main"))],
        );
        assert!(links.pull_requests.is_empty(), "{links:?}");
    }

    #[test]
    fn a_checked_out_fork_links_through_its_upstream() {
        let from_fork = pr(PR_URL, "acme/api", "ada/api", "fix");
        let links = links_for(
            GH,
            &from_fork,
            &[
                remote("origin", "git@github.com:acme/api.git"),
                remote("ada", "https://github.com/ada/api.git"),
            ],
            &[wt("/code/api-review", Some("ada-fix"), Some("ada/fix"))],
        );
        assert_eq!(
            linked_paths(&links),
            vec![PathBuf::from("/code/api-review")]
        );
    }

    #[test]
    fn an_ssh_alias_links_and_an_unknown_https_host_does_not() {
        let pr = pr(PR_URL, "acme/api", "acme/api", "feature");
        let alias = links_for(
            GH,
            &pr,
            &[remote("origin", "git@github-work:acme/api.git")],
            &[wt("/code/api", Some("feature"), Some("origin/feature"))],
        );
        assert_eq!(linked_paths(&alias), vec![PathBuf::from("/code/api")]);
        let gitlab = links_for(
            GH,
            &pr,
            &[remote("origin", "https://gitlab.com/acme/api.git")],
            &[wt("/code/api", Some("feature"), Some("origin/feature"))],
        );
        assert!(gitlab.pull_requests.is_empty(), "{gitlab:?}");
    }

    #[test]
    fn hosts_must_agree_when_both_are_known() {
        let pr = pr(PR_URL, "acme/api", "acme/api", "feature");
        let links = links_for(
            GH,
            &pr,
            &[remote("origin", "git@bitbucket.org:acme/api.git")],
            &[wt("/code/api", Some("feature"), None)],
        );
        assert!(links.pull_requests.is_empty(), "{links:?}");
    }

    #[test]
    fn two_clones_both_link_main_worktrees_first_then_by_path() {
        let pr = pr(PR_URL, "acme/api", "acme/api", "feature");
        let remotes = [remote("origin", "git@github.com:acme/api.git")];
        let b_refs = [wt("/code/b/api", Some("feature"), None)];
        let a_refs = [
            wt("/code/a/api-wt", Some("feature"), None),
            wt("/code/a/api", Some("feature"), None),
        ];
        let repos = [
            RepoInput {
                repo_id: Path::new("/code/b/api/.git"),
                main_worktree: Path::new("/code/b/api"),
                worktrees: &b_refs,
                remotes: &remotes,
            },
            RepoInput {
                repo_id: Path::new("/code/a/api/.git"),
                main_worktree: Path::new("/code/a/api"),
                worktrees: &a_refs,
                remotes: &remotes,
            },
        ];
        let links = link_pull_requests(
            &[PullRequestInput {
                provider: GH,
                role: PullRequestRole::Authored,
                row: &pr,
            }],
            &repos,
        );
        assert_eq!(
            linked_paths(&links),
            vec![
                PathBuf::from("/code/a/api"),
                PathBuf::from("/code/b/api"),
                PathBuf::from("/code/a/api-wt"),
            ]
        );
        assert_eq!(
            links.pull_requests[0].worktrees[0].repo_id,
            PathBuf::from("/code/a/api/.git")
        );
    }

    #[test]
    fn detached_worktrees_empty_urls_and_empty_head_repositories_link_nothing() {
        let remotes = [remote("origin", "git@github.com:acme/api.git")];
        let detached = links_for(
            GH,
            &pr(PR_URL, "acme/api", "acme/api", "feature"),
            &remotes,
            &[wt("/code/api", None, Some("origin/feature"))],
        );
        assert!(detached.pull_requests.is_empty());
        let no_url = links_for(
            GH,
            &pr("", "acme/api", "acme/api", "feature"),
            &remotes,
            &[wt("/code/api", Some("feature"), None)],
        );
        assert!(no_url.pull_requests.is_empty());
        let deleted_fork = links_for(
            GH,
            &pr(PR_URL, "acme/api", "", "feature"),
            &remotes,
            &[wt("/code/api", Some("feature"), None)],
        );
        assert!(deleted_fork.pull_requests.is_empty());
        let no_remotes = links_for(
            GH,
            &pr(PR_URL, "acme/api", "acme/api", "feature"),
            &[],
            &[wt("/code/api", Some("feature"), None)],
        );
        assert!(no_remotes.pull_requests.is_empty());
    }

    #[test]
    fn a_worktrees_pull_requests_keep_the_input_order() {
        let remotes = [
            remote("origin", "git@github.com:acme/api.git"),
            remote("bb", "git@bitbucket.org:acme/api.git"),
        ];
        let refs = [wt("/code/api", Some("feature"), None)];
        let bitbucket = pr(
            "https://bitbucket.org/acme/api/pull-requests/1",
            "acme/api",
            "acme/api",
            "feature",
        );
        let mine = pr(
            "https://github.com/acme/api/pull/2",
            "acme/api",
            "acme/api",
            "feature",
        );
        let theirs = pr(
            "https://github.com/acme/api/pull/3",
            "acme/api",
            "acme/api",
            "feature",
        );
        let repo = RepoInput {
            repo_id: Path::new("/code/api/.git"),
            main_worktree: Path::new("/code/api"),
            worktrees: &refs,
            remotes: &remotes,
        };
        let links = link_pull_requests(
            &[
                PullRequestInput {
                    provider: BB,
                    role: PullRequestRole::Authored,
                    row: &bitbucket,
                },
                PullRequestInput {
                    provider: GH,
                    role: PullRequestRole::Authored,
                    row: &mine,
                },
                PullRequestInput {
                    provider: GH,
                    role: PullRequestRole::ReviewRequested,
                    row: &theirs,
                },
            ],
            &[repo],
        );
        assert_eq!(links.worktrees.len(), 1);
        let listed: Vec<(PullRequestProvider, PullRequestRole, &str)> = links.worktrees[0]
            .pull_requests
            .iter()
            .map(|p| (p.provider, p.role, p.url.as_str()))
            .collect();
        assert_eq!(
            listed,
            vec![
                (
                    BB,
                    PullRequestRole::Authored,
                    "https://bitbucket.org/acme/api/pull-requests/1"
                ),
                (
                    GH,
                    PullRequestRole::Authored,
                    "https://github.com/acme/api/pull/2"
                ),
                (
                    GH,
                    PullRequestRole::ReviewRequested,
                    "https://github.com/acme/api/pull/3"
                ),
            ]
        );
        assert_eq!(
            links
                .pull_requests
                .iter()
                .map(|p| p.url.as_str())
                .collect::<Vec<_>>(),
            vec![
                "https://bitbucket.org/acme/api/pull-requests/1",
                "https://github.com/acme/api/pull/2",
                "https://github.com/acme/api/pull/3",
            ]
        );
    }

    #[test]
    fn a_linked_entry_carries_the_rows_signals() {
        let mut row = pr(PR_URL, "acme/api", "acme/api", "feature");
        row.id = 42;
        row.title = "Add rate limits".to_string();
        row.draft = true;
        row.checks = Some(ChecksState::Failing);
        row.conflicting = true;
        row.review = Some(ReviewSummary {
            approvals: 1,
            changes_requested: 0,
            pending: 2,
        });
        let links = links_for(
            GH,
            &row,
            &[remote("origin", "git@github.com:acme/api.git")],
            &[wt("/code/api", Some("feature"), None)],
        );
        assert_eq!(
            links.worktrees,
            vec![WorktreePullRequests {
                worktree_path: PathBuf::from("/code/api"),
                pull_requests: vec![LinkedPullRequest {
                    provider: GH,
                    role: PullRequestRole::Authored,
                    id: 42,
                    title: "Add rate limits".to_string(),
                    url: PR_URL.to_string(),
                    repo_full_name: "acme/api".to_string(),
                    draft: true,
                    checks: Some(ChecksState::Failing),
                    conflicting: true,
                    review: row.review,
                }],
            }]
        );
        assert_eq!(
            links.pull_requests[0].worktrees[0].branch.as_deref(),
            Some("feature")
        );
    }

    #[test]
    fn the_links_are_camel_case_on_the_wire() {
        let links = links_for(
            GH,
            &pr(PR_URL, "acme/api", "acme/api", "feature"),
            &[remote("origin", "git@github.com:acme/api.git")],
            &[wt("/code/api", Some("feature"), None)],
        );
        let wire = serde_json::to_value(&links).unwrap();
        assert!(wire["worktrees"][0]["worktreePath"].is_string());
        assert_eq!(wire["worktrees"][0]["pullRequests"][0]["role"], "authored");
        assert_eq!(
            wire["worktrees"][0]["pullRequests"][0]["provider"],
            "github"
        );
        assert!(wire["pullRequests"][0]["worktrees"][0]["repoId"].is_string());
        assert_eq!(
            serde_json::to_value(PullRequestRole::ReviewRequested).unwrap(),
            "reviewRequested"
        );
    }
}
