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
