use std::fs::{self, OpenOptions};
use std::io::Write;

use crate::context::{Context, REPOSITORY_EXCLUDE_PATTERN};
use crate::error::{MadmError, Result};

const EXCLUDE_COMMENT: &str = "# madm: never track the repository's own data";

pub fn ensure_repository_excluded(context: &Context) -> Result<()> {
    let path = context.repository().join("info").join("exclude");
    let existing = match fs::read(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(MadmError::io("read repository exclude file", error)),
    };

    if has_exact_line(&existing, REPOSITORY_EXCLUDE_PATTERN.as_bytes()) {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| MadmError::io("create repository info directory", error))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| MadmError::io("open repository exclude file", error))?;

    if !existing.is_empty() && !existing.ends_with(b"\n") {
        file.write_all(b"\n")
            .map_err(|error| MadmError::io("update repository exclude file", error))?;
    }
    writeln!(file, "{EXCLUDE_COMMENT}")
        .and_then(|_| writeln!(file, "{REPOSITORY_EXCLUDE_PATTERN}"))
        .map_err(|error| MadmError::io("update repository exclude file", error))
}

fn has_exact_line(content: &[u8], expected: &[u8]) -> bool {
    content.split(|byte| *byte == b'\n').any(|line| {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        line == expected
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_line_detection_accepts_lf_and_crlf() {
        assert!(has_exact_line(b"one\n/pattern/\n", b"/pattern/"));
        assert!(has_exact_line(b"one\r\n/pattern/\r\n", b"/pattern/"));
        assert!(!has_exact_line(b"/pattern/extra\n", b"/pattern/"));
    }
}
