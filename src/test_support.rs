use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::configure_new_repository;
use crate::context::Context;
use crate::git::{self, Git};
use crate::temp::TemporaryDirectory;

pub struct GitFixture {
    _temporary: TemporaryDirectory,
    context: Context,
}

impl GitFixture {
    pub fn new() -> Self {
        let temporary = TemporaryDirectory::create("madm-unit-test").unwrap();
        let home = temporary.path().join("home");
        fs::create_dir(&home).unwrap();
        let context = Context::from_home(home).unwrap();
        fs::create_dir_all(context.repository_parent()).unwrap();
        let status = git::raw_status([
            OsString::from("init"),
            OsString::from("--bare"),
            context.repository().as_os_str().to_owned(),
        ])
        .unwrap();
        assert!(status.success());
        configure_new_repository(&context).unwrap();
        let git = Git::new(&context);
        git.checked_status(
            [
                OsStr::new("config"),
                OsStr::new("user.name"),
                OsStr::new("Test"),
            ],
            "configure test user",
        )
        .unwrap();
        git.checked_status(
            [
                OsStr::new("config"),
                OsStr::new("user.email"),
                OsStr::new("test@example.com"),
            ],
            "configure test email",
        )
        .unwrap();
        Self {
            _temporary: temporary,
            context,
        }
    }

    pub fn git(&self) -> Git<'_> {
        Git::new(&self.context)
    }

    pub fn home(&self) -> &Path {
        self.context.home()
    }

    pub fn write(&self, path: &str, content: &str) {
        let path = self.home().join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    pub fn read(&self, path: &str) -> String {
        fs::read_to_string(self.home().join(path)).unwrap()
    }

    pub fn path(&self, path: &str) -> PathBuf {
        self.home().join(path)
    }

    pub fn commit_all(&self, message: &str) {
        let git = self.git();
        git.checked_status([OsStr::new("add"), OsStr::new("-A")], "stage test files")
            .unwrap();
        git.checked_status(
            [OsStr::new("commit"), OsStr::new("-m"), OsStr::new(message)],
            "commit test files",
        )
        .unwrap();
    }

    pub fn current_branch(&self) -> String {
        self.git()
            .text(
                [
                    OsStr::new("symbolic-ref"),
                    OsStr::new("--short"),
                    OsStr::new("HEAD"),
                ],
                "read test branch",
            )
            .unwrap()
    }
}
