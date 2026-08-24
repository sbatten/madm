mod support;

use std::ffi::OsString;
use std::fs;

use support::{RemoteRepository, TestEnvironment, assert_success, stderr, stdout};

#[test]
fn init_passthrough_list_and_safety_are_git_native() {
    let environment = TestEnvironment::new("init");
    let init = environment.madm(&["init"]);
    assert_success(&init);
    assert!(environment.repository().is_dir());

    environment.write_home(".sample", "sample");
    environment.madm_success(&["add", "-A"]);
    let list = environment.madm_success(&["list"]);
    assert_eq!(stdout(&list).trim(), ".sample");

    let work_tree = environment.madm_success(&["config", "--get", "core.worktree"]);
    assert_eq!(
        std::path::PathBuf::from(stdout(&work_tree).trim()),
        environment.home()
    );
    let untracked = environment.madm_success(&["config", "--get", "status.showUntrackedFiles"]);
    assert_eq!(stdout(&untracked).trim(), "no");

    let exclude_path = environment.repository().join("info").join("exclude");
    let mut exclude = fs::read_to_string(&exclude_path).unwrap();
    exclude.push_str("# user rule\n*.private\n");
    fs::write(&exclude_path, exclude).unwrap();
    environment.madm_success(&["status", "--short"]);
    environment.madm_success(&["status", "--short"]);
    let exclude = fs::read_to_string(exclude_path).unwrap();
    assert!(exclude.contains("# user rule\n*.private\n"));
    assert_eq!(exclude.matches("/.local/share/madm/repo.git/").count(), 1);

    let clean = environment.madm(&["clean", "-fdx"]);
    assert!(!clean.status.success());
    assert!(stderr(&clean).contains("disabled"));
    let second_init = environment.madm(&["init"]);
    assert!(!second_init.status.success());
    assert!(stderr(&second_init).contains("refusing to overwrite"));
}

#[test]
fn existing_fixed_path_bare_repository_works_without_conversion() {
    let environment = TestEnvironment::new("existing");
    fs::create_dir_all(environment.repository().parent().unwrap()).unwrap();
    assert_success(&environment.git([
        OsString::from("init"),
        OsString::from("--bare"),
        environment.repository().as_os_str().to_owned(),
    ]));
    environment.write_home(".existing", "value");

    environment.madm_success(&["add", ".existing"]);
    let bare = environment.madm_success(&["config", "--bool", "core.bare"]);
    assert_eq!(stdout(&bare).trim(), "true");
    let exclude =
        fs::read_to_string(environment.repository().join("info").join("exclude")).unwrap();
    assert!(exclude.contains("/.local/share/madm/repo.git/"));
}

#[test]
fn clone_installs_missing_files_and_keeps_differences_unstaged() {
    let environment = TestEnvironment::new("clone");
    let remote = RemoteRepository::new(
        &environment,
        &[
            (".missing", "remote missing"),
            (".same", "same"),
            (".different", "remote"),
        ],
    );
    environment.write_home(".same", "same");
    environment.write_home(".different", "local");
    environment.write_home("home-only", "untouched");

    let clone = environment.madm(&["clone", &remote.bare.to_string_lossy()]);
    assert_success(&clone);
    assert!(stdout(&clone).contains("Kept differing local files"));
    assert_eq!(environment.read_home(".missing"), "remote missing");
    assert_eq!(environment.read_home(".same"), "same");
    assert_eq!(environment.read_home(".different"), "local");
    assert_eq!(environment.read_home("home-only"), "untouched");

    let status = environment.madm_success(&["status", "--short", "-uno"]);
    assert_eq!(stdout(&status).trim(), "M .different");
    let staged = environment.madm_success(&["diff", "--cached", "--name-only"]);
    assert!(stdout(&staged).is_empty());
}

#[test]
fn clone_rejects_a_tree_that_tracks_the_reserved_repository_path() {
    let environment = TestEnvironment::new("reserved");
    let remote = RemoteRepository::new(
        &environment,
        &[(".local/share/madm/repo.git/evil", "do not install")],
    );

    let clone = environment.madm(&["clone", &remote.bare.to_string_lossy()]);
    assert!(!clone.status.success());
    assert!(stderr(&clone).contains("reserved path"));
    assert!(!environment.repository().exists());
    assert!(
        !environment
            .home()
            .join(".local/share/madm/repo.git/evil")
            .exists()
    );
}

#[test]
fn failed_clone_removes_only_the_setup_directories_it_created() {
    let environment = TestEnvironment::new("clone-cleanup");
    let missing_remote = environment.root().join("does-not-exist.git");

    let clone = environment.madm(&["clone", &missing_remote.to_string_lossy()]);
    assert!(!clone.status.success());
    assert!(!environment.repository().exists());
    assert!(!environment.home().join(".local").exists());
}

#[test]
fn compare_fetches_and_preserves_head_index_and_work_tree() {
    let environment = TestEnvironment::new("compare");
    let remote = RemoteRepository::new(&environment, &[(".config", "base")]);
    environment.madm_success(&["clone", &remote.bare.to_string_lossy()]);

    environment.write_home(".config", "staged");
    environment.madm_success(&["add", ".config"]);
    environment.write_home(".config", "effective");
    remote.write_commit(&environment, ".remote", "new", "remote change");

    let before_head = environment.madm_success(&["rev-parse", "HEAD"]).stdout;
    let before_status = environment
        .madm_success(&["status", "--porcelain=v1", "-z", "-uno"])
        .stdout;
    let compare = environment.madm(&["compare"]);
    assert_success(&compare);
    let output = stdout(&compare);
    assert!(output.contains("0 commit(s) ahead and 1 commit(s) behind"));
    assert!(output.contains("+effective"));
    assert!(output.contains("diff --git a/.remote b/.remote"));
    assert_eq!(environment.read_home(".config"), "effective");
    assert_eq!(
        environment.madm_success(&["rev-parse", "HEAD"]).stdout,
        before_head
    );
    assert_eq!(
        environment
            .madm_success(&["status", "--porcelain=v1", "-z", "-uno"])
            .stdout,
        before_status
    );
}

#[test]
fn sync_merges_divergent_commits_and_pushes_automatically() {
    let environment = TestEnvironment::new("sync-divergent");
    let remote = RemoteRepository::new(&environment, &[(".shared", "base")]);
    environment.madm_success(&["clone", &remote.bare.to_string_lossy()]);
    environment.configure_madm_user();

    environment.write_home(".local-change", "local");
    environment.madm_success(&["add", ".local-change"]);
    environment.madm_success(&["commit", "-m", "local change"]);
    remote.write_commit(&environment, ".remote-change", "remote", "remote change");

    let sync = environment.madm(&["sync"]);
    assert_success(&sync);
    assert!(stdout(&sync).contains("Synchronized"));
    let parents = environment.git([
        OsString::from("--git-dir"),
        remote.bare.as_os_str().to_owned(),
        OsString::from("rev-list"),
        OsString::from("--parents"),
        OsString::from("-n"),
        OsString::from("1"),
        OsString::from("HEAD"),
    ]);
    assert_success(&parents);
    assert_eq!(stdout(&parents).split_whitespace().count(), 3);
    let remote_local = environment.git([
        OsString::from("--git-dir"),
        remote.bare.as_os_str().to_owned(),
        OsString::from("show"),
        OsString::from("HEAD:.local-change"),
    ]);
    assert_success(&remote_local);
    assert_eq!(stdout(&remote_local).trim(), "local");
}

#[test]
fn sync_rejects_dirty_tracked_state_with_exact_commit_steps() {
    let environment = TestEnvironment::new("sync-dirty");
    let remote = RemoteRepository::new(&environment, &[(".config", "base")]);
    environment.madm_success(&["clone", &remote.bare.to_string_lossy()]);
    environment.write_home(".config", "dirty");

    let sync = environment.madm(&["sync"]);
    assert!(!sync.status.success());
    let error = stderr(&sync);
    assert!(error.contains("madm add -u"));
    assert!(error.contains("madm commit"));
    assert!(error.contains("madm sync"));
    assert_eq!(environment.read_home(".config"), "dirty");
}

#[test]
fn sync_stops_before_overwriting_untracked_home_content() {
    let environment = TestEnvironment::new("sync-collision");
    let remote = RemoteRepository::new(&environment, &[(".base", "base")]);
    environment.madm_success(&["clone", &remote.bare.to_string_lossy()]);
    environment.write_home(".collision", "local");
    remote.write_commit(&environment, ".collision", "remote", "remote collision");
    let before_head = environment.madm_success(&["rev-parse", "HEAD"]).stdout;

    let sync = environment.madm(&["sync"]);
    assert!(!sync.status.success());
    let error = stderr(&sync);
    assert!(error.contains("upstream would overwrite untracked home content"));
    assert!(error.contains("madm add"));
    assert_eq!(environment.read_home(".collision"), "local");
    assert_eq!(
        environment.madm_success(&["rev-parse", "HEAD"]).stdout,
        before_head
    );
    let merge_head = environment.madm(&["rev-parse", "--verify", "MERGE_HEAD"]);
    assert!(!merge_head.status.success());
}

#[test]
fn sync_collision_guidance_uses_force_for_an_ignored_file() {
    let environment = TestEnvironment::new("sync-ignored-collision");
    let remote = RemoteRepository::new(&environment, &[(".base", "base")]);
    environment.madm_success(&["clone", &remote.bare.to_string_lossy()]);
    environment.write_home(".gitignore", ".collision\n");
    environment.write_home(".collision", "local");
    remote.write_commit(&environment, ".collision", "remote", "remote collision");

    let sync = environment.madm(&["sync"]);
    assert!(!sync.status.success());
    let error = stderr(&sync);
    assert!(error.contains("madm add -f -- \".collision\""));
    assert_eq!(environment.read_home(".collision"), "local");
}

#[test]
fn sync_rejects_an_incoming_tree_that_uses_the_repository_path() {
    let environment = TestEnvironment::new("sync-reserved");
    let remote = RemoteRepository::new(&environment, &[(".base", "base")]);
    environment.madm_success(&["clone", &remote.bare.to_string_lossy()]);
    remote.write_commit(
        &environment,
        ".local/share/madm/repo.git/evil",
        "do not install",
        "reserved path",
    );

    let sync = environment.madm(&["sync"]);
    assert!(!sync.status.success());
    assert!(stderr(&sync).contains("reserved path"));
    assert!(!environment.repository().join("evil").exists());
    assert!(
        !environment
            .madm(&["rev-parse", "--verify", "MERGE_HEAD"])
            .status
            .success()
    );
}

#[test]
fn noninteractive_sync_preserves_a_conflicted_merge_for_resolve() {
    let environment = TestEnvironment::new("sync-conflict");
    let remote = RemoteRepository::new(&environment, &[(".config", "base")]);
    environment.madm_success(&["clone", &remote.bare.to_string_lossy()]);
    environment.configure_madm_user();
    environment.write_home(".config", "local");
    environment.madm_success(&["add", ".config"]);
    environment.madm_success(&["commit", "-m", "local"]);
    remote.write_commit(&environment, ".config", "remote", "remote");

    let sync = environment.madm(&["sync"]);
    assert_eq!(sync.status.code(), Some(2));
    assert!(stdout(&sync).contains("Merge conflicts still need resolution"));
    assert!(
        environment
            .madm(&["rev-parse", "--verify", "MERGE_HEAD"])
            .status
            .success()
    );

    let resolve = environment.madm(&["resolve"]);
    assert_eq!(resolve.status.code(), Some(2));
    assert!(stdout(&resolve).contains(".config"));
}

#[test]
fn sync_publishes_a_new_branch_and_sets_its_upstream() {
    let environment = TestEnvironment::new("sync-first-push");
    let remote = environment.root().join("empty.git");
    assert_success(&environment.git([
        OsString::from("init"),
        OsString::from("--bare"),
        remote.as_os_str().to_owned(),
    ]));
    environment.madm_success(&["init"]);
    environment.configure_madm_user();
    environment.write_home(".first", "first");
    environment.madm_success(&["add", ".first"]);
    environment.madm_success(&["commit", "-m", "first"]);
    environment.madm_success(&["remote", "add", "origin", &remote.to_string_lossy()]);

    let sync = environment.madm(&["sync"]);
    assert_success(&sync);
    let upstream =
        environment.madm_success(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
    assert_eq!(
        stdout(&upstream).trim(),
        format!(
            "origin/{}",
            stdout(&environment.madm_success(&["symbolic-ref", "--short", "HEAD"])).trim()
        )
    );
    let remote_file = environment.git([
        OsString::from("--git-dir"),
        remote.as_os_str().to_owned(),
        OsString::from("show"),
        OsString::from("HEAD:.first"),
    ]);
    assert_success(&remote_file);
    assert_eq!(stdout(&remote_file).trim(), "first");
}

#[test]
fn help_and_version_work_without_a_repository() {
    let environment = TestEnvironment::new("help");
    let help = environment.madm(&["--help"]);
    assert_success(&help);
    assert!(stdout(&help).contains("minimal, Git-native"));
    let version = environment.madm(&["--version"]);
    assert_success(&version);
    assert!(stdout(&version).starts_with("madm "));
}
