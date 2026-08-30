//! Git operations for team workspaces, via libgit2.
//!
//! A workspace directory is a team workspace exactly when it is a git
//! repository. Per-user files (history, locks, temp files) are excluded
//! through a committed `.gitignore`.

use std::path::Path;

use git2::{Cred, PushOptions, RemoteCallbacks, Repository, Signature};
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

/// Adds `origin` pointing at `url` and pushes `main`, authenticating with
/// the GitHub token.
pub fn add_origin_and_push(dir: &Path, url: &str, token: &str) -> Result<(), GitError> {
    let repo = Repository::open(dir)?;
    let mut remote = match repo.find_remote("origin") {
        Ok(remote) => remote,
        Err(_) => repo.remote("origin", url)?,
    };
    let mut callbacks = RemoteCallbacks::new();
    let token = token.to_string();
    callbacks.credentials(move |_url, _username, _allowed| {
        Cred::userpass_plaintext("x-access-token", &token)
    });
    let mut options = PushOptions::new();
    options.remote_callbacks(callbacks);
    remote.push(
        &[format!("refs/heads/{BRANCH}:refs/heads/{BRANCH}")],
        Some(&mut options),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/git.rs"]
mod tests;
