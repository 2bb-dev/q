//! Git operations for team workspaces, via libgit2.
//!
//! A workspace directory is a team workspace exactly when it is a git
//! repository. Per-user files (history, locks, temp files) are excluded
//! through a committed `.gitignore`.

use std::path::Path;

use git2::build::CheckoutBuilder;
use git2::{Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository, Signature};
use thiserror::Error;

const GITIGNORE: &str = "history.json\n.lock\n*.json.tmp\n";
const BRANCH: &str = "main";

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git error: {0}")]
    Git(#[from] git2::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Whether the workspace directory is a team workspace (a git repository).
pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Initializes a team repository in an existing workspace directory:
/// `git init` on `main`, a `.gitignore` for per-user files, and an initial
/// commit of the workspace content.
pub fn init_team_repo(dir: &Path, author: &str) -> Result<(), GitError> {
    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head(&format!("refs/heads/{BRANCH}"));
    let repo = Repository::init_opts(dir, &options)?;
    std::fs::write(dir.join(".gitignore"), GITIGNORE)?;
    commit_all_in(&repo, author, "Initialize q workspace")?;
    Ok(())
}

/// Stages every change and commits it. Returns false when there was
/// nothing to commit.
pub fn commit_all(dir: &Path, author: &str, message: &str) -> Result<bool, GitError> {
    let repo = Repository::open(dir)?;
    commit_all_in(&repo, author, message)
}

fn commit_all_in(repo: &Repository, author: &str, message: &str) -> Result<bool, GitError> {
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"].iter(), None)?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(_) => None,
    };
    if let Some(parent) = &parent {
        if parent.tree_id() == tree_id {
            return Ok(false);
        }
    }
    let tree = repo.find_tree(tree_id)?;
    let signature = Signature::now(author, &format!("{author}@users.noreply.github.com"))?;
    let parents: Vec<_> = parent.iter().collect();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;
    Ok(true)
}

fn token_callbacks(token: &str) -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();
    let token = token.to_string();
    callbacks.credentials(move |_url, _username, _allowed| {
        Cred::userpass_plaintext("x-access-token", &token)
    });
    callbacks
}

/// Adds `origin` pointing at `url` and pushes `main`, authenticating with
/// the GitHub token.
pub fn add_origin_and_push(dir: &Path, url: &str, token: &str) -> Result<(), GitError> {
    let repo = Repository::open(dir)?;
    if repo.find_remote("origin").is_err() {
        repo.remote("origin", url)?;
    }
    push(dir, token)
}

/// Pushes `main` to `origin`.
pub fn push(dir: &Path, token: &str) -> Result<(), GitError> {
    let repo = Repository::open(dir)?;
    let mut remote = repo.find_remote("origin")?;
    let mut options = PushOptions::new();
    options.remote_callbacks(token_callbacks(token));
    remote.push(
        &[format!("refs/heads/{BRANCH}:refs/heads/{BRANCH}")],
        Some(&mut options),
    )?;
    Ok(())
}

/// Clones a team workspace repository into `dir`.
pub fn clone_repo(url: &str, dir: &Path, token: &str) -> Result<(), GitError> {
    let mut options = FetchOptions::new();
    options.remote_callbacks(token_callbacks(token));
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(options);
    builder.clone(url, dir)?;
    Ok(())
}

/// URL of the `origin` remote, when the workspace has one.
pub fn origin_url(dir: &Path) -> Option<String> {
    let repo = Repository::open(dir).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    remote.url().map(str::to_string)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    UpToDate,
    Updated,
}

/// Fetches `origin/main` and merges it into the local branch. Per-prompt
/// files make most merges clean; when both sides changed the same file, the
/// side with the newer `updated_at` (falling back to `created_at`) wins, and
/// a deletion beats a modification.
pub fn fetch_and_merge(dir: &Path, token: &str, author: &str) -> Result<SyncOutcome, GitError> {
    let repo = Repository::open(dir)?;
    let mut remote = repo.find_remote("origin")?;
    let mut options = FetchOptions::new();
    options.remote_callbacks(token_callbacks(token));
    remote.fetch(&[BRANCH], Some(&mut options), None)?;

    let fetch_head = match repo.find_reference("FETCH_HEAD") {
        Ok(reference) => reference,
        Err(_) => return Ok(SyncOutcome::UpToDate),
    };
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
    let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;

    if analysis.is_up_to_date() {
        return Ok(SyncOutcome::UpToDate);
    }
    if analysis.is_fast_forward() {
        let refname = format!("refs/heads/{BRANCH}");
        let mut reference = repo.find_reference(&refname)?;
        reference.set_target(fetch_commit.id(), "fast-forward")?;
        repo.set_head(&refname)?;
        repo.checkout_head(Some(CheckoutBuilder::default().force()))?;
        return Ok(SyncOutcome::Updated);
    }

    // Normal merge with newest-wins conflict resolution.
    let mut checkout = CheckoutBuilder::new();
    checkout.allow_conflicts(true);
    repo.merge(&[&fetch_commit], None, Some(&mut checkout))?;
    let mut index = repo.index()?;
    if index.has_conflicts() {
        let conflicts: Vec<_> = index.conflicts()?.collect::<Result<_, _>>()?;
        for conflict in conflicts {
            let path_bytes = conflict
                .our
                .as_ref()
                .or(conflict.their.as_ref())
                .or(conflict.ancestor.as_ref())
                .map(|entry| entry.path.clone())
                .unwrap_or_default();
            let rel = String::from_utf8_lossy(&path_bytes).to_string();
            match (&conflict.our, &conflict.their) {
                // One side deleted the file: the deletion wins (a popped
                // prompt must not resurrect).
                (None, _) | (_, None) => {
                    index.remove_path(Path::new(&rel))?;
                    let _ = std::fs::remove_file(dir.join(&rel));
                }
                (Some(our), Some(their)) => {
                    let our_blob = repo.find_blob(our.id)?;
                    let their_blob = repo.find_blob(their.id)?;
                    let chosen = if newer_wins(our_blob.content(), their_blob.content()) {
                        our_blob.content().to_vec()
                    } else {
                        their_blob.content().to_vec()
                    };
                    std::fs::write(dir.join(&rel), &chosen)?;
                    index.remove_path(Path::new(&rel))?;
                    index.add_path(Path::new(&rel))?;
                }
            }
        }
    }
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let signature = Signature::now(author, &format!("{author}@users.noreply.github.com"))?;
    let head = repo.head()?.peel_to_commit()?;
    let theirs = repo.find_commit(fetch_commit.id())?;
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Merge remote changes",
        &tree,
        &[&head, &theirs],
    )?;
    repo.cleanup_state()?;
    repo.checkout_head(Some(CheckoutBuilder::default().force()))?;
    Ok(SyncOutcome::Updated)
}

/// True when `ours` should win: its `updated_at` (falling back to
/// `created_at`) is strictly newer than theirs.
fn newer_wins(ours: &[u8], theirs: &[u8]) -> bool {
    fn stamp(bytes: &[u8]) -> Option<chrono::DateTime<chrono::Utc>> {
        let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        let stamp = value
            .get("updated_at")
            .or_else(|| value.get("created_at"))
            .and_then(|stamp| stamp.as_str())?;
        chrono::DateTime::parse_from_rfc3339(stamp)
            .ok()
            .map(|parsed| parsed.with_timezone(&chrono::Utc))
    }
    match (stamp(ours), stamp(theirs)) {
        (Some(ours), Some(theirs)) => ours > theirs,
        // Undecidable: let the remote side win for determinism.
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSyncState {
    /// No uncommitted changes and local main matches origin/main.
    Synced,
    /// Uncommitted changes or commits origin does not have yet.
    Pending,
}

/// Local-only view of whether the workspace has unsynced work. Never
/// touches the network.
pub fn sync_state(dir: &Path) -> Result<LocalSyncState, GitError> {
    let repo = Repository::open(dir)?;
    let mut options = git2::StatusOptions::new();
    options.include_untracked(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let dirty = statuses.iter().any(|status| !status.status().is_ignored());
    if dirty {
        return Ok(LocalSyncState::Pending);
    }
    let local = repo.refname_to_id(&format!("refs/heads/{BRANCH}"))?;
    match repo.refname_to_id(&format!("refs/remotes/origin/{BRANCH}")) {
        Ok(remote) if remote == local => Ok(LocalSyncState::Synced),
        Ok(_) => Ok(LocalSyncState::Pending),
        Err(_) => Ok(LocalSyncState::Pending),
    }
}

#[cfg(test)]
#[path = "../tests/unit/git.rs"]
mod tests;
