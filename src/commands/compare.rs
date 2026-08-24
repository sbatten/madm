use std::ffi::OsStr;

use crate::error::{MadmError, Result};
use crate::git::{self, Git};

use super::prepare_repository;
use super::upstream;

pub fn run() -> Result<i32> {
    let context = prepare_repository()?;
    let git = Git::new(&context);
    if !upstream::head_exists(&git)? {
        return Err(MadmError::new(
            "the current branch has no commits; commit tracked files before comparing",
        ));
    }
    let upstream = upstream::resolve(&git, false)?;
    let range = format!("HEAD...{}", upstream.reference);
    let counts = git.text(
        [
            OsStr::new("rev-list"),
            OsStr::new("--left-right"),
            OsStr::new("--count"),
            OsStr::new(&range),
        ],
        "calculate upstream difference",
    )?;
    let (ahead, behind) = parse_counts(&counts)?;

    println!(
        "Branch '{}' is {ahead} commit(s) ahead and {behind} commit(s) behind {}/{}.",
        upstream.local_branch, upstream.remote, upstream.remote_branch
    );
    println!(
        "--- tracked home state compared with {}/{} ---",
        upstream.remote, upstream.remote_branch
    );

    let status = git.status([
        OsStr::new("diff"),
        OsStr::new("--patch"),
        OsStr::new(&upstream.reference),
        OsStr::new("--"),
    ])?;
    if status.success() {
        Ok(0)
    } else {
        Err(MadmError::with_code(
            "Git could not compare the tracked home state with upstream",
            git::status_code(status),
        ))
    }
}

fn parse_counts(counts: &str) -> Result<(u64, u64)> {
    let mut fields = counts.split_whitespace();
    let ahead = fields
        .next()
        .and_then(|field| field.parse::<u64>().ok())
        .ok_or_else(|| MadmError::new(format!("unexpected Git ahead/behind output: {counts:?}")))?;
    let behind = fields
        .next()
        .and_then(|field| field.parse::<u64>().ok())
        .ok_or_else(|| MadmError::new(format!("unexpected Git ahead/behind output: {counts:?}")))?;
    if fields.next().is_some() {
        return Err(MadmError::new(format!(
            "unexpected Git ahead/behind output: {counts:?}"
        )));
    }
    Ok((ahead, behind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ahead_and_behind_counts() {
        assert_eq!(parse_counts("2\t3").unwrap(), (2, 3));
        assert!(parse_counts("2").is_err());
        assert!(parse_counts("two 3").is_err());
    }
}
