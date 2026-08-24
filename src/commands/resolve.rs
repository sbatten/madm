use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::error::{MadmError, Result};
use crate::git::Git;
use crate::path::nul_paths;
use crate::prompt::Interaction;

use super::prepare_repository;

const PENDING_RESOLUTION_EXIT_CODE: i32 = 2;

pub fn run(interaction: &mut dyn Interaction) -> Result<i32> {
    let context = prepare_repository()?;
    let git = Git::new(&context);
    match current_operation(&context, &git)? {
        Operation::Merge => {}
        operation => return Err(non_merge_operation_error(operation)),
    }
    if resolve_merge(&git, interaction)? {
        Ok(0)
    } else {
        Ok(PENDING_RESOLUTION_EXIT_CODE)
    }
}

pub fn resolve_merge(git: &Git<'_>, interaction: &mut dyn Interaction) -> Result<bool> {
    let unresolved = unresolved_paths(git)?;
    if unresolved.is_empty() {
        finish_merge(git)?;
        return Ok(true);
    }

    if !interaction.is_interactive() {
        print_pending(&unresolved);
        return Ok(false);
    }

    for (index, path) in unresolved.iter().enumerate() {
        loop {
            if !is_unresolved(git, path)? {
                break;
            }
            let response = interaction.prompt(&format!(
                "{} is unresolved [Enter=skip, l=local, u=upstream, m=mergetool, q=quit]: ",
                path.display()
            ))?;
            match response.to_ascii_lowercase().as_str() {
                "" | "s" => break,
                "q" => {
                    print_pending(&unresolved[index..]);
                    return Ok(false);
                }
                "l" => {
                    select_side(git, path, Side::Local)?;
                    println!("Resolved {} with the local side.", path.display());
                    break;
                }
                "u" => {
                    select_side(git, path, Side::Upstream)?;
                    println!("Resolved {} with the upstream side.", path.display());
                    break;
                }
                "m" => {
                    let status = git.status([
                        OsStr::new("mergetool"),
                        OsStr::new("--no-prompt"),
                        OsStr::new("--"),
                        path.as_os_str(),
                    ])?;
                    if status.success() && !is_unresolved(git, path)? {
                        println!(
                            "Resolved {} with the configured merge tool.",
                            path.display()
                        );
                        break;
                    }
                    eprintln!(
                        "madm: the merge tool did not resolve {}; choose another action",
                        path.display()
                    );
                }
                _ => eprintln!("madm: choose Enter, l, u, m, or q"),
            }
        }
    }

    let remaining = unresolved_paths(git)?;
    if !remaining.is_empty() {
        print_pending(&remaining);
        return Ok(false);
    }

    finish_merge(git)?;
    Ok(true)
}

pub fn merge_is_active(git: &Git<'_>) -> Result<bool> {
    pseudo_ref_exists(git, "MERGE_HEAD")
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    None,
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

pub fn current_operation(context: &Context, git: &Git<'_>) -> Result<Operation> {
    if merge_is_active(git)? {
        return Ok(Operation::Merge);
    }

    if context.repository().join("rebase-merge").exists()
        || context.repository().join("rebase-apply").exists()
    {
        return Ok(Operation::Rebase);
    }
    if pseudo_ref_exists(git, "CHERRY_PICK_HEAD")? {
        return Ok(Operation::CherryPick);
    }
    if pseudo_ref_exists(git, "REVERT_HEAD")? {
        return Ok(Operation::Revert);
    }
    Ok(Operation::None)
}

pub fn non_merge_operation_error(operation: Operation) -> MadmError {
    match operation {
        Operation::Rebase => MadmError::new(
            "a rebase is in progress; madm resolve handles merges only\n\
             Resolve paths with 'madm status' and 'madm add -- <path>', then run:\n  \
             madm rebase --continue",
        ),
        Operation::CherryPick => MadmError::new(
            "a cherry-pick is in progress; madm resolve handles merges only\n\
             Resolve paths with 'madm status' and 'madm add -- <path>', then run:\n  \
             madm cherry-pick --continue",
        ),
        Operation::Revert => MadmError::new(
            "a revert is in progress; madm resolve handles merges only\n\
             Resolve paths with 'madm status' and 'madm add -- <path>', then run:\n  \
             madm revert --continue",
        ),
        Operation::None => MadmError::new("no Git merge is in progress"),
        Operation::Merge => MadmError::new("a Git merge is already in progress"),
    }
}

fn unresolved_paths(git: &Git<'_>) -> Result<Vec<PathBuf>> {
    nul_paths(git.checked_output(
        [
            OsStr::new("diff"),
            OsStr::new("--name-only"),
            OsStr::new("--diff-filter=U"),
            OsStr::new("-z"),
            OsStr::new("--"),
        ],
        "list unresolved merge paths",
    )?)
}

fn is_unresolved(git: &Git<'_>, path: &Path) -> Result<bool> {
    let output = git.checked_output(
        [
            OsStr::new("ls-files"),
            OsStr::new("--unmerged"),
            OsStr::new("-z"),
            OsStr::new("--"),
            path.as_os_str(),
        ],
        "inspect unresolved merge path",
    )?;
    Ok(!output.is_empty())
}

#[derive(Copy, Clone)]
enum Side {
    Local,
    Upstream,
}

fn select_side(git: &Git<'_>, path: &Path, side: Side) -> Result<()> {
    let entries = git.checked_output(
        [
            OsStr::new("ls-files"),
            OsStr::new("--unmerged"),
            OsStr::new("-z"),
            OsStr::new("--"),
            path.as_os_str(),
        ],
        "inspect merge stages",
    )?;
    let stages = parse_unmerged_stages(&entries)?;
    let (stage, checkout_side, label) = match side {
        Side::Local => (2, "--ours", "local"),
        Side::Upstream => (3, "--theirs", "upstream"),
    };

    if stages.contains(&stage) {
        git.checked_status(
            [
                OsStr::new("checkout"),
                OsStr::new(checkout_side),
                OsStr::new("--"),
                path.as_os_str(),
            ],
            &format!("check out the {label} side of {}", path.display()),
        )?;
        git.checked_status(
            [OsStr::new("add"), OsStr::new("--"), path.as_os_str()],
            &format!("stage the {label} resolution for {}", path.display()),
        )
    } else {
        git.checked_status(
            [
                OsStr::new("rm"),
                OsStr::new("-f"),
                OsStr::new("--"),
                path.as_os_str(),
            ],
            &format!("stage the {label} deletion for {}", path.display()),
        )
    }
}

fn parse_unmerged_stages(entries: &[u8]) -> Result<HashSet<u8>> {
    let mut stages = HashSet::new();
    for entry in entries
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
    {
        let tab = entry
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| MadmError::new("unexpected Git unmerged-index output without a path"))?;
        let header = std::str::from_utf8(&entry[..tab])
            .map_err(|_| MadmError::new("unexpected non-UTF-8 Git index metadata"))?;
        let stage = header
            .split_whitespace()
            .nth(2)
            .and_then(|value| value.parse::<u8>().ok())
            .ok_or_else(|| MadmError::new("unexpected Git unmerged-index stage"))?;
        stages.insert(stage);
    }
    Ok(stages)
}

fn pseudo_ref_exists(git: &Git<'_>, reference: &str) -> Result<bool> {
    let output = git.output([
        OsStr::new("rev-parse"),
        OsStr::new("--verify"),
        OsStr::new("--quiet"),
        OsStr::new(reference),
    ])?;
    if output.status.success() {
        Ok(true)
    } else if matches!(output.status.code(), Some(1 | 128)) {
        Ok(false)
    } else {
        Err(crate::git::output_error(
            &format!("inspect Git state '{reference}'"),
            output,
        ))
    }
}

fn finish_merge(git: &Git<'_>) -> Result<()> {
    git.checked_status(
        [OsStr::new("commit"), OsStr::new("--no-edit")],
        "finish the merge commit",
    )?;
    println!("Finished the merge commit.");
    Ok(())
}

fn print_pending(paths: &[PathBuf]) {
    println!("Merge conflicts still need resolution:");
    for path in paths {
        println!("  {}", path.display());
    }
    println!("Run 'madm resolve' in an interactive terminal to continue.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::prompt::FakeInteraction;
    use crate::test_support::GitFixture;

    #[test]
    fn parses_unmerged_index_stages() {
        let entries = b"100644 aaaaa 1\tfile\0\
                        100644 bbbbb 2\tfile\0\
                        100644 ccccc 3\tfile\0";
        assert_eq!(
            parse_unmerged_stages(entries).unwrap(),
            HashSet::from([1, 2, 3])
        );
    }

    #[test]
    fn skipped_conflict_is_resumable_and_local_resolution_is_committed() {
        let fixture = conflicting_fixture();
        let git = fixture.git();
        let mut skip = FakeInteraction::interactive(&[""]);

        assert!(!resolve_merge(&git, &mut skip).unwrap());
        assert!(merge_is_active(&git).unwrap());
        assert!(is_unresolved(&git, Path::new(".config")).unwrap());

        let mut local = FakeInteraction::interactive(&["l"]);
        assert!(resolve_merge(&git, &mut local).unwrap());
        assert!(!merge_is_active(&git).unwrap());
        assert_eq!(fixture.read(".config"), "local");
        let parents = git
            .text(
                [
                    OsStr::new("rev-list"),
                    OsStr::new("--parents"),
                    OsStr::new("-n"),
                    OsStr::new("1"),
                    OsStr::new("HEAD"),
                ],
                "inspect merge parents",
            )
            .unwrap();
        assert_eq!(parents.split_whitespace().count(), 3);
    }

    #[test]
    fn upstream_deletion_removes_the_file() {
        let fixture = GitFixture::new();
        fixture.write(".config", "base");
        fixture.commit_all("base");
        let original = fixture.current_branch();
        let git = fixture.git();
        git.checked_status(
            [
                OsStr::new("checkout"),
                OsStr::new("-b"),
                OsStr::new("upstream"),
            ],
            "create upstream branch",
        )
        .unwrap();
        fs::remove_file(fixture.path(".config")).unwrap();
        fixture.commit_all("upstream deletes");
        git.checked_status(
            [OsStr::new("checkout"), OsStr::new(&original)],
            "restore local branch",
        )
        .unwrap();
        fixture.write(".config", "local");
        fixture.commit_all("local modifies");
        let status = git
            .status([OsStr::new("merge"), OsStr::new("upstream")])
            .unwrap();
        assert!(!status.success());

        let mut upstream = FakeInteraction::interactive(&["u"]);
        assert!(resolve_merge(&git, &mut upstream).unwrap());
        assert!(!fixture.path(".config").exists());
    }

    fn conflicting_fixture() -> GitFixture {
        let fixture = GitFixture::new();
        fixture.write(".config", "base");
        fixture.commit_all("base");
        let original = fixture.current_branch();
        let git = fixture.git();
        git.checked_status(
            [
                OsStr::new("checkout"),
                OsStr::new("-b"),
                OsStr::new("upstream"),
            ],
            "create upstream branch",
        )
        .unwrap();
        fixture.write(".config", "upstream");
        fixture.commit_all("upstream changes");
        git.checked_status(
            [OsStr::new("checkout"), OsStr::new(&original)],
            "restore local branch",
        )
        .unwrap();
        fixture.write(".config", "local");
        fixture.commit_all("local changes");
        let status = git
            .status([OsStr::new("merge"), OsStr::new("upstream")])
            .unwrap();
        assert!(!status.success());
        fixture
    }
}
