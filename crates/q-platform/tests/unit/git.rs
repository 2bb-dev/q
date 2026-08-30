use super::*;
use tempfile::TempDir;

fn seeded_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("workspace.json"), "{}\n").unwrap();
    std::fs::write(dir.path().join("history.json"), "{}\n").unwrap();
    dir
}

#[test]
fn init_team_repo_makes_the_dir_a_repo_with_an_initial_commit() {
    let dir = seeded_dir();
    assert!(!is_repo(dir.path()));

    init_team_repo(dir.path(), "octocat").unwrap();

    assert!(is_repo(dir.path()));
    let repo = git2::Repository::open(dir.path()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message(), Some("Initialize q workspace"));
    assert_eq!(head.author().name(), Some("octocat"));
    assert_eq!(
        repo.head().unwrap().shorthand(),
        Some("main"),
        "initial branch should be main"
    );
}

#[test]
fn per_user_files_are_ignored_and_never_committed() {
    let dir = seeded_dir();
    std::fs::write(dir.path().join(".lock"), "").unwrap();
    init_team_repo(dir.path(), "octocat").unwrap();

    let repo = git2::Repository::open(dir.path()).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    assert!(tree.get_name("workspace.json").is_some());
    assert!(tree.get_name(".gitignore").is_some());
    assert!(tree.get_name("history.json").is_none());
    assert!(tree.get_name(".lock").is_none());

    // Later history changes never show up as commits either.
    std::fs::write(dir.path().join("history.json"), "{\"changed\":1}\n").unwrap();
    assert!(!commit_all(dir.path(), "octocat", "noop").unwrap());
}

#[test]
fn commit_all_commits_changes_and_reports_noops() {
    let dir = seeded_dir();
    init_team_repo(dir.path(), "octocat").unwrap();

    assert!(!commit_all(dir.path(), "octocat", "nothing yet").unwrap());
    std::fs::write(dir.path().join("workspace.json"), "{\"name\":\"x\"}\n").unwrap();
    assert!(commit_all(dir.path(), "octocat", "update").unwrap());
    assert!(!commit_all(dir.path(), "octocat", "again").unwrap());
}

#[test]
fn push_to_a_local_bare_remote_roundtrips() {
    let dir = seeded_dir();
    init_team_repo(dir.path(), "octocat").unwrap();
    let remote = TempDir::new().unwrap();
    git2::Repository::init_bare(remote.path()).unwrap();

    add_origin_and_push(dir.path(), remote.path().to_str().unwrap(), "unused").unwrap();

    let bare = git2::Repository::open_bare(remote.path()).unwrap();
    assert!(bare.find_branch("main", git2::BranchType::Local).is_ok());
}

// --- Sync: fetch/merge/push against local bare origins (no network) ---

fn bare_origin() -> TempDir {
    let remote = TempDir::new().unwrap();
    let mut options = git2::RepositoryInitOptions::new();
    options.bare(true);
    options.initial_head("refs/heads/main");
    git2::Repository::init_opts(remote.path(), &options).unwrap();
    remote
}

fn team_clone(origin: &TempDir) -> TempDir {
    let dir = TempDir::new().unwrap();
    // clone_repo needs an empty target; clone into a subdir then use it.
    let target = dir.path().join("ws");
    clone_repo(origin.path().to_str().unwrap(), &target, "unused").unwrap();
    dir
}

fn seeded_origin() -> TempDir {
    let seed = seeded_dir();
    init_team_repo(seed.path(), "octocat").unwrap();
    let origin = bare_origin();
    add_origin_and_push(seed.path(), origin.path().to_str().unwrap(), "unused").unwrap();
    origin
}

#[test]
fn concurrent_disjoint_changes_merge_cleanly() {
    let origin = seeded_origin();
    let a = team_clone(&origin);
    let b = team_clone(&origin);
    let (a, b) = (a.path().join("ws"), b.path().join("ws"));

    std::fs::write(a.join("from-a.json"), "{\"v\":\"a\"}\n").unwrap();
    commit_all(&a, "alice", "a adds").unwrap();
    push(&a, "unused").unwrap();

    std::fs::write(b.join("from-b.json"), "{\"v\":\"b\"}\n").unwrap();
    commit_all(&b, "bob", "b adds").unwrap();
    assert_eq!(
        fetch_and_merge(&b, "unused", "bob").unwrap(),
        SyncOutcome::Updated
    );
    push(&b, "unused").unwrap();

    assert_eq!(
        fetch_and_merge(&a, "unused", "alice").unwrap(),
        SyncOutcome::Updated
    );
    assert!(a.join("from-a.json").exists());
    assert!(a.join("from-b.json").exists());
    assert!(b.join("from-a.json").exists());
    assert!(b.join("from-b.json").exists());
}

#[test]
fn same_file_conflict_resolves_to_the_newer_updated_at() {
    let origin = seeded_origin();
    let a = team_clone(&origin);
    let b = team_clone(&origin);
    let (a, b) = (a.path().join("ws"), b.path().join("ws"));

    let older = r#"{"text":"older","updated_at":"2026-01-01T10:00:00Z"}"#;
    let newer = r#"{"text":"newer","updated_at":"2026-01-01T10:00:01Z"}"#;

    std::fs::write(a.join("prompt.json"), older).unwrap();
    commit_all(&a, "alice", "a edit").unwrap();
    push(&a, "unused").unwrap();

    std::fs::write(b.join("prompt.json"), newer).unwrap();
    commit_all(&b, "bob", "b edit").unwrap();
    fetch_and_merge(&b, "unused", "bob").unwrap();
    assert!(std::fs::read_to_string(b.join("prompt.json"))
        .unwrap()
        .contains("newer"));
    push(&b, "unused").unwrap();

    fetch_and_merge(&a, "unused", "alice").unwrap();
    assert!(std::fs::read_to_string(a.join("prompt.json"))
        .unwrap()
        .contains("newer"));
}

#[test]
fn deletion_beats_modification_in_conflicts() {
    let origin = seeded_origin();
    let seeder = team_clone(&origin);
    let seeder = seeder.path().join("ws");
    std::fs::write(
        seeder.join("prompt.json"),
        r#"{"text":"seed","updated_at":"2026-01-01T09:00:00Z"}"#,
    )
    .unwrap();
    commit_all(&seeder, "carol", "seed prompt").unwrap();
    push(&seeder, "unused").unwrap();

    let a = team_clone(&origin);
    let b = team_clone(&origin);
    let (a, b) = (a.path().join("ws"), b.path().join("ws"));

    // A pops (deletes) the prompt.
    std::fs::remove_file(a.join("prompt.json")).unwrap();
    commit_all(&a, "alice", "pop").unwrap();
    push(&a, "unused").unwrap();

    // B edits the same prompt concurrently.
    std::fs::write(
        b.join("prompt.json"),
        r#"{"text":"edited","updated_at":"2026-01-01T11:00:00Z"}"#,
    )
    .unwrap();
    commit_all(&b, "bob", "edit").unwrap();
    fetch_and_merge(&b, "unused", "bob").unwrap();

    assert!(
        !b.join("prompt.json").exists(),
        "popped prompt must not resurrect"
    );
}

#[test]
fn sync_state_tracks_dirty_ahead_and_synced() {
    let origin = seeded_origin();
    let a = team_clone(&origin);
    let a = a.path().join("ws");

    assert_eq!(sync_state(&a).unwrap(), LocalSyncState::Synced);
    std::fs::write(a.join("new.json"), "{}\n").unwrap();
    assert_eq!(sync_state(&a).unwrap(), LocalSyncState::Pending);
    commit_all(&a, "alice", "add").unwrap();
    assert_eq!(sync_state(&a).unwrap(), LocalSyncState::Pending);
    push(&a, "unused").unwrap();
    assert_eq!(sync_state(&a).unwrap(), LocalSyncState::Synced);
}
