mod clone;
mod compare;
mod init;
mod resolve;
mod sync;
mod upstream;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::PathBuf;

use crate::cli::{self, CliCommand};
use crate::context::Context;
use crate::error::{MadmError, Result};
use crate::exclude::ensure_repository_excluded;
use crate::git::{self, Git};
use crate::prompt::Interaction;

pub fn execute(command: Result<CliCommand>, interaction: &mut dyn Interaction) -> Result<i32> {
    match command? {
        CliCommand::Help(command) => {
            print!("{}", cli::help(command.as_deref())?);
            Ok(0)
        }
        CliCommand::GitHelp(command) => {
            let status = git::raw_status([OsStr::new("help"), command.as_os_str()])?;
            Ok(git::status_code(status))
        }
        CliCommand::Version => {
            println!("madm {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        CliCommand::Clean => Err(MadmError::new(
            "'madm clean' is disabled because it could delete untracked files throughout your home directory; invoke raw Git deliberately if needed",
        )),
        CliCommand::Init => init::run(),
        CliCommand::Clone(url) => clone::run(url, interaction),
        CliCommand::Compare => compare::run(),
        CliCommand::Sync => sync::run(interaction),
        CliCommand::Resolve => resolve::run(interaction),
        CliCommand::List(args) => {
            let mut git_args = vec![OsString::from("ls-files")];
            git_args.extend(args);
            passthrough(git_args)
        }
        CliCommand::Git(args) => passthrough(args),
    }
}

fn passthrough(args: Vec<OsString>) -> Result<i32> {
    let context = prepare_repository()?;
    Git::new(&context).passthrough(&args)
}

pub(crate) fn prepare_repository() -> Result<Context> {
    let context = Context::discover()?;
    match fs::symlink_metadata(context.repository()) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return Err(MadmError::new(format!(
                "the madm repository path is not a directory: {}",
                context.repository().display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(MadmError::new(format!(
                "no madm repository found at {}; run 'madm init' or 'madm clone <url>'",
                context.repository().display()
            )));
        }
        Err(error) => return Err(MadmError::io("inspect madm repository path", error)),
    }

    let git = Git::new(&context);
    git.checked_output(
        [OsStr::new("rev-parse"), OsStr::new("--git-dir")],
        "validate repository",
    )?;
    ensure_repository_excluded(&context)?;
    ensure_reserved_path_is_untracked(&git)?;
    Ok(context)
}

pub(crate) fn ensure_reserved_path_is_untracked(git: &Git<'_>) -> Result<()> {
    let tracked = git.checked_output(
        [
            OsStr::new("ls-files"),
            OsStr::new("-z"),
            OsStr::new("--"),
            OsStr::new(crate::context::REPOSITORY_RELATIVE_GIT_PATH),
        ],
        "check reserved repository path",
    )?;
    if tracked.is_empty() {
        Ok(())
    } else {
        Err(MadmError::new(
            "the repository tracks files below the reserved path '.local/share/madm/repo.git'; remove them from the Git index before using madm",
        ))
    }
}

pub(crate) fn ensure_revision_avoids_reserved_path(git: &Git<'_>, revision: &str) -> Result<()> {
    let tracked = git.checked_output(
        [
            OsStr::new("ls-tree"),
            OsStr::new("-r"),
            OsStr::new("--name-only"),
            OsStr::new("-z"),
            OsStr::new(revision),
            OsStr::new("--"),
            OsStr::new(crate::context::REPOSITORY_RELATIVE_GIT_PATH),
        ],
        "check reserved repository path in incoming tree",
    )?;
    if tracked.is_empty() {
        Ok(())
    } else {
        Err(MadmError::new(format!(
            "refusing to use Git tree {revision} because it tracks files below the reserved path '{}'",
            crate::context::REPOSITORY_RELATIVE_GIT_PATH
        )))
    }
}

pub(crate) fn configure_new_repository(context: &Context) -> Result<()> {
    let git = Git::new(context);
    git.checked_status(
        [
            OsStr::new("config"),
            OsStr::new("core.bare"),
            OsStr::new("false"),
        ],
        "configure repository work tree",
    )?;
    git.checked_status(
        [
            OsStr::new("config"),
            OsStr::new("core.worktree"),
            context.home().as_os_str(),
        ],
        "configure repository work tree",
    )?;
    git.checked_status(
        [
            OsStr::new("config"),
            OsStr::new("status.showUntrackedFiles"),
            OsStr::new("no"),
        ],
        "configure untracked-file status",
    )?;
    ensure_repository_excluded(context)?;
    Ok(())
}

pub(crate) fn cleanup_failed_setup(
    context: &Context,
    created_parents: &[PathBuf],
    original: MadmError,
) -> Result<i32> {
    let mut cleanup_errors = Vec::new();
    match fs::symlink_metadata(context.repository()) {
        Ok(metadata) if metadata.is_dir() => {
            if let Err(error) = fs::remove_dir_all(context.repository()) {
                cleanup_errors.push(format!(
                    "remove {}: {error}",
                    context.repository().display()
                ));
            }
        }
        Ok(_) => {
            if let Err(error) = fs::remove_file(context.repository()) {
                cleanup_errors.push(format!(
                    "remove {}: {error}",
                    context.repository().display()
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => cleanup_errors.push(format!(
            "inspect {}: {error}",
            context.repository().display()
        )),
    }

    for path in created_parents.iter().rev() {
        if let Err(error) = fs::remove_dir(path) {
            cleanup_errors.push(format!("remove {}: {error}", path.display()));
        }
    }

    if cleanup_errors.is_empty() {
        Err(original)
    } else {
        Err(MadmError::new(format!(
            "{}; setup cleanup was incomplete: {}",
            original.message(),
            cleanup_errors.join("; ")
        )))
    }
}
