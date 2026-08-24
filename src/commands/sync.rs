use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::error::{MadmError, Result};
use crate::git::{self, Git};
use crate::path::{existing_path_blocker, nul_paths};
use crate::prompt::Interaction;

use super::resolve::{self, Operation};
use super::upstream::{self, Upstream};
use super::{ensure_revision_avoids_reserved_path, prepare_repository};

const PENDING_RESOLUTION_EXIT_CODE: i32 = 2;

pub fn run(interaction: &mut dyn Interaction) -> Result<i32> {
    let context = prepare_repository()?;
    let git = Git::new(&context);

    match resolve::current_operation(&context, &git)? {
        Operation::None => {}
        Operation::Merge => {
            if !resolve::resolve_merge(&git, interaction)? {
                return Ok(PENDING_RESOLUTION_EXIT_CODE);
            }
        }
        operation => return Err(resolve::non_merge_operation_error(operation)),
    }

    if !upstream::head_exists(&git)? {
        return Err(MadmError::new(
            "the current branch has no commits\n\
             Create the first commit, then sync:\n  \
             madm add -- <path>\n  \
             madm commit\n  \
             madm sync",
        ));
    }
    require_clean_tracked_state(&git)?;

    let upstream = upstream::resolve(&git, true)?;
    if upstream.remote == "." {
        return Err(MadmError::new(
            "the current branch tracks another local branch; madm sync requires a remote upstream",
        ));
    }

    if upstream.exists {
        ensure_revision_avoids_reserved_path(&git, &upstream.reference)?;
        reject_untracked_collisions(&context, &git, &upstream)?;
        let status = git.status([
            OsStr::new("merge"),
            OsStr::new("--ff"),
            OsStr::new("--no-edit"),
            OsStr::new(&upstream.reference),
        ])?;
        if !status.success() {
            if resolve::merge_is_active(&git)? {
                if !resolve::resolve_merge(&git, interaction)? {
                    return Ok(PENDING_RESOLUTION_EXIT_CODE);
                }
            } else {
                return Err(MadmError::with_code(
                    "Git could not merge upstream; no files were forcefully overwritten",
                    git::status_code(status),
                ));
            }
        }
    }

    push(&git, &upstream)?;
    println!(
        "Synchronized '{}' with {}/{}.",
        upstream.local_branch, upstream.remote, upstream.remote_branch
    );
    Ok(0)
}

fn require_clean_tracked_state(git: &Git<'_>) -> Result<()> {
    let status = git.checked_output(
        [
            OsStr::new("status"),
            OsStr::new("--porcelain=v1"),
            OsStr::new("-z"),
            OsStr::new("--untracked-files=no"),
        ],
        "inspect tracked home state",
    )?;
    if status.is_empty() {
        return Ok(());
    }

    let summary = git.text(
        [
            OsStr::new("status"),
            OsStr::new("--short"),
            OsStr::new("--untracked-files=no"),
        ],
        "summarize tracked home changes",
    )?;
    Err(MadmError::new(format!(
        "sync requires a clean tracked work tree and index\n\
         Tracked changes:\n{summary}\n\
         Commit them, then retry:\n  \
         madm add -u\n  \
         madm commit\n  \
         madm sync"
    )))
}

#[derive(Debug)]
struct Collision {
    remote_path: PathBuf,
    local_blocker: PathBuf,
    ignored: bool,
}

fn reject_untracked_collisions(
    context: &crate::context::Context,
    git: &Git<'_>,
    upstream: &Upstream,
) -> Result<()> {
    let added = nul_paths(git.checked_output(
        [
            OsStr::new("diff"),
            OsStr::new("--name-only"),
            OsStr::new("--diff-filter=A"),
            OsStr::new("--no-renames"),
            OsStr::new("-z"),
            OsStr::new("HEAD"),
            OsStr::new(&upstream.reference),
            OsStr::new("--"),
        ],
        "inspect upstream-added paths",
    )?)?;

    let mut seen = HashSet::new();
    let mut collisions = Vec::new();
    for remote_path in added {
        if let Some(local_blocker) = existing_path_blocker(context.home(), &remote_path)? {
            if seen.insert(local_blocker.clone()) {
                collisions.push(Collision {
                    ignored: is_ignored(git, &local_blocker)?,
                    remote_path,
                    local_blocker,
                });
            }
        }
    }

    if collisions.is_empty() {
        return Ok(());
    }

    let details = collisions
        .iter()
        .map(|collision| {
            format!(
                "  upstream {} is blocked by local {}",
                collision.remote_path.display(),
                collision.local_blocker.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let add_commands = collisions
        .iter()
        .map(|collision| {
            let force = if collision.ignored { " -f" } else { "" };
            format!(
                "  madm add{force} -- \"{}\"",
                escape_double_quotes(&collision.local_blocker)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Err(MadmError::new(format!(
        "upstream would overwrite untracked home content; nothing was changed\n\
         Collisions:\n{details}\n\
         Track the local content so Git can merge it normally:\n\
         {add_commands}\n  \
         madm commit\n  \
         madm sync"
    )))
}

fn is_ignored(git: &Git<'_>, path: &Path) -> Result<bool> {
    let output = git.output([
        OsStr::new("check-ignore"),
        OsStr::new("--quiet"),
        OsStr::new("--"),
        path.as_os_str(),
    ])?;
    if output.status.success() {
        Ok(true)
    } else if output.status.code() == Some(1) {
        Ok(false)
    } else {
        Err(crate::git::output_error(
            &format!("check ignore rules for {}", path.display()),
            output,
        ))
    }
}

fn escape_double_quotes(path: &Path) -> String {
    path.to_string_lossy().replace('"', "\\\"")
}

fn push(git: &Git<'_>, upstream: &Upstream) -> Result<()> {
    let refspec = format!("HEAD:refs/heads/{}", upstream.remote_branch);
    let mut arguments = vec![OsString::from("push")];
    if upstream.inferred {
        arguments.push(OsString::from("--set-upstream"));
    }
    arguments.push(OsString::from(&upstream.remote));
    arguments.push(OsString::from(refspec));

    let status = git.status(arguments)?;
    if status.success() {
        Ok(())
    } else {
        Err(MadmError::with_code(
            "push was rejected or failed; no force push was attempted. Run 'madm sync' again after reviewing the remote update",
            git::status_code(status),
        ))
    }
}
