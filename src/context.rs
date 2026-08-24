use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{MadmError, Result};

pub const REPOSITORY_RELATIVE_GIT_PATH: &str = ".local/share/madm/repo.git";
pub const REPOSITORY_EXCLUDE_PATTERN: &str = "/.local/share/madm/repo.git/";

#[derive(Clone, Debug)]
pub struct Context {
    home: PathBuf,
    repository: PathBuf,
}

impl Context {
    pub fn discover() -> Result<Self> {
        Self::from_home(resolve_home()?)
    }

    pub fn from_home(home: PathBuf) -> Result<Self> {
        if !home.is_absolute() {
            return Err(MadmError::new(format!(
                "home directory must be an absolute path: {}",
                home.display()
            )));
        }
        if !home.is_dir() {
            return Err(MadmError::new(format!(
                "home directory does not exist or is not a directory: {}",
                home.display()
            )));
        }
        let repository = home
            .join(".local")
            .join("share")
            .join("madm")
            .join("repo.git");
        Ok(Self { home, repository })
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn repository(&self) -> &Path {
        &self.repository
    }

    pub fn repository_parent(&self) -> &Path {
        self.repository
            .parent()
            .expect("the fixed repository always has a parent")
    }

    pub fn create_repository_parent(&self) -> Result<Vec<PathBuf>> {
        let candidates = [
            self.home.join(".local"),
            self.home.join(".local").join("share"),
            self.repository_parent().to_owned(),
        ];
        let mut created = Vec::new();

        for path in candidates {
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    cleanup_empty_directories(&created);
                    return Err(MadmError::new(format!(
                        "repository parent path is not a directory: {}",
                        path.display()
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    match fs::create_dir(&path) {
                        Ok(()) => created.push(path),
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            if !path.is_dir() {
                                cleanup_empty_directories(&created);
                                return Err(MadmError::new(format!(
                                    "repository parent path is not a directory: {}",
                                    path.display()
                                )));
                            }
                        }
                        Err(error) => {
                            cleanup_empty_directories(&created);
                            return Err(MadmError::io(
                                &format!("create repository parent directory {}", path.display()),
                                error,
                            ));
                        }
                    }
                }
                Err(error) => {
                    cleanup_empty_directories(&created);
                    return Err(MadmError::io(
                        &format!("inspect repository parent directory {}", path.display()),
                        error,
                    ));
                }
            }
        }
        Ok(created)
    }
}

fn cleanup_empty_directories(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        let _ = fs::remove_dir(path);
    }
}

fn resolve_home() -> Result<PathBuf> {
    let candidates = home_candidates();
    candidates
        .into_iter()
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            MadmError::new(
                "could not determine the home directory from the operating-system environment",
            )
        })
}

#[cfg(windows)]
fn home_candidates() -> Vec<OsString> {
    let mut candidates = Vec::new();
    if let Some(profile) = env::var_os("USERPROFILE") {
        candidates.push(profile);
    }
    if let Some(home) = env::var_os("HOME") {
        candidates.push(home);
    }
    if let (Some(drive), Some(path)) = (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) {
        let mut combined = drive;
        combined.push(path);
        candidates.push(combined);
    }
    candidates
}

#[cfg(not(windows))]
fn home_candidates() -> Vec<OsString> {
    env::var_os("HOME").into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_repository_is_below_home_on_every_platform() {
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\Users\test")
        } else {
            PathBuf::from("/home/test")
        };
        let context = Context {
            repository: root
                .join(".local")
                .join("share")
                .join("madm")
                .join("repo.git"),
            home: root.clone(),
        };
        assert_eq!(
            context.repository().strip_prefix(root).unwrap(),
            Path::new(".local")
                .join("share")
                .join("madm")
                .join("repo.git")
        );
    }
}
