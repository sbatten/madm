use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub struct TestEnvironment {
    root: PathBuf,
    home: PathBuf,
    global_config: PathBuf,
}

impl TestEnvironment {
    pub fn new(name: &str) -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "madm-integration-{name}-{}-{sequence}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();
        Self {
            global_config: root.join("global.gitconfig"),
            root,
            home,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn repository(&self) -> PathBuf {
        self.home
            .join(".local")
            .join("share")
            .join("madm")
            .join("repo.git")
    }

    pub fn write_home(&self, path: &str, content: &str) {
        let path = self.home.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    pub fn read_home(&self, path: &str) -> String {
        fs::read_to_string(self.home.join(path)).unwrap()
    }

    pub fn madm(&self, args: &[&str]) -> Output {
        self.madm_in(self.root(), args)
    }

    pub fn madm_in(&self, directory: &Path, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_madm"));
        command.args(args).current_dir(directory);
        self.configure(&mut command);
        command.output().unwrap()
    }

    pub fn git<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("git");
        command.args(args).current_dir(self.root());
        self.configure(&mut command);
        command.output().unwrap()
    }

    pub fn git_in<I, S>(&self, directory: &Path, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("git");
        command.arg("-C").arg(directory).args(args);
        self.configure(&mut command);
        command.output().unwrap()
    }

    pub fn madm_success(&self, args: &[&str]) -> Output {
        let output = self.madm(args);
        assert_success(&output);
        output
    }

    pub fn configure_madm_user(&self) {
        assert_success(&self.madm(&["config", "user.name", "Test User"]));
        assert_success(&self.madm(&["config", "user.email", "test@example.com"]));
    }

    fn configure(&self, command: &mut Command) {
        command
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE");
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub struct RemoteRepository {
    pub bare: PathBuf,
    pub seed: PathBuf,
}

impl RemoteRepository {
    pub fn new(environment: &TestEnvironment, files: &[(&str, &str)]) -> Self {
        let bare = environment.root().join("remote.git");
        let seed = environment.root().join("seed");
        fs::create_dir_all(&seed).unwrap();
        assert_success(&environment.git([
            OsString::from("init"),
            OsString::from("--bare"),
            bare.as_os_str().to_owned(),
        ]));
        assert_success(&environment.git_in(&seed, ["init"]));
        assert_success(&environment.git_in(&seed, ["config", "user.name", "Test User"]));
        assert_success(&environment.git_in(&seed, ["config", "user.email", "test@example.com"]));
        for (path, content) in files {
            let destination = seed.join(path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(destination, content).unwrap();
        }
        if !files.is_empty() {
            assert_success(&environment.git_in(&seed, ["add", "-A"]));
            assert_success(&environment.git_in(&seed, ["commit", "-m", "base"]));
            assert_success(&environment.git_in(
                &seed,
                [
                    OsStr::new("remote"),
                    OsStr::new("add"),
                    OsStr::new("origin"),
                    bare.as_os_str(),
                ],
            ));
            assert_success(&environment.git_in(&seed, ["push", "-u", "origin", "HEAD"]));
        }
        Self { bare, seed }
    }

    pub fn write_commit(
        &self,
        environment: &TestEnvironment,
        path: &str,
        content: &str,
        message: &str,
    ) {
        let destination = self.seed.join(path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(destination, content).unwrap();
        assert_success(&environment.git_in(&self.seed, ["add", "-A"]));
        assert_success(&environment.git_in(&self.seed, ["commit", "-m", message]));
        assert_success(&environment.git_in(&self.seed, ["push"]));
    }
}

pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
