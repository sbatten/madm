use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{MadmError, Result};

static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub struct TemporaryDirectory {
    path: PathBuf,
    removed: bool,
}

impl TemporaryDirectory {
    pub fn create(prefix: &str) -> Result<Self> {
        let parent = std::env::temp_dir();
        for _ in 0..100 {
            let sequence = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let name = format!("{prefix}-{}-{sequence}", std::process::id());
            let path = parent.join(name);
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        removed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(MadmError::io("create temporary directory", error));
                }
            }
        }
        Err(MadmError::new(
            "could not allocate a unique temporary directory",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn remove(mut self) -> Result<()> {
        fs::remove_dir_all(&self.path)
            .map_err(|error| MadmError::io("remove temporary directory", error))?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if !self.removed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
