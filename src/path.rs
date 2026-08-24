use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::error::{MadmError, Result};

pub fn nul_paths(output: Vec<u8>) -> Result<Vec<PathBuf>> {
    output
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(path_from_git_bytes)
        .collect()
}

pub fn existing_path_blocker(home: &Path, repository_path: &Path) -> Result<Option<PathBuf>> {
    let mut relative = PathBuf::new();
    let components = repository_path.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::Normal(value) => relative.push(value),
            _ => {
                return Err(MadmError::new(format!(
                    "Git returned a path outside the work tree: {}",
                    repository_path.display()
                )));
            }
        }

        let candidate = home.join(&relative);
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) => {
                let final_component = index + 1 == components.len();
                if final_component || !metadata.is_dir() {
                    return Ok(Some(relative));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(MadmError::io(
                    &format!("inspect local path {}", candidate.display()),
                    error,
                ));
            }
        }
    }
    Ok(None)
}

#[cfg(unix)]
fn path_from_git_bytes(bytes: &[u8]) -> Result<PathBuf> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

#[cfg(windows)]
fn path_from_git_bytes(bytes: &[u8]) -> Result<PathBuf> {
    let value = String::from_utf8(bytes.to_vec())
        .map_err(|_| MadmError::new("Git returned a path that is not valid UTF-8 on Windows"))?;
    Ok(PathBuf::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temp::TemporaryDirectory;

    #[test]
    fn parses_nul_delimited_paths_without_splitting_whitespace() {
        let paths = nul_paths(b"one path\0line\nbreak\0".to_vec()).unwrap();
        assert_eq!(
            paths,
            vec![PathBuf::from("one path"), PathBuf::from("line\nbreak")]
        );
    }

    #[test]
    fn detects_exact_and_parent_file_collisions() {
        let temporary = TemporaryDirectory::create("madm-collision-test").unwrap();
        fs::write(temporary.path().join("exact"), b"local").unwrap();
        fs::write(temporary.path().join("parent"), b"local").unwrap();

        assert_eq!(
            existing_path_blocker(temporary.path(), Path::new("exact")).unwrap(),
            Some(PathBuf::from("exact"))
        );
        assert_eq!(
            existing_path_blocker(temporary.path(), Path::new("parent/child")).unwrap(),
            Some(PathBuf::from("parent"))
        );
        assert_eq!(
            existing_path_blocker(temporary.path(), Path::new("missing/child")).unwrap(),
            None
        );
    }
}
