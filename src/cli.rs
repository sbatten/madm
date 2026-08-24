use std::ffi::{OsStr, OsString};

use crate::error::{MadmError, Result};

#[derive(Debug, Eq, PartialEq)]
pub enum CliCommand {
    Help(Option<OsString>),
    GitHelp(OsString),
    Version,
    Init,
    Clone(OsString),
    Compare,
    Sync,
    Resolve,
    List(Vec<OsString>),
    Clean,
    Git(Vec<OsString>),
}

pub fn parse(args: Vec<OsString>) -> Result<CliCommand> {
    let Some(first) = args.first() else {
        return Ok(CliCommand::Help(None));
    };

    match first.to_str() {
        Some("-h" | "--help") => no_extra(args, CliCommand::Help(None), "madm --help"),
        Some("-V" | "--version" | "version") => {
            no_extra(args, CliCommand::Version, "madm --version")
        }
        Some("help") => parse_help(args),
        Some("init") => parse_no_args(args, "init", CliCommand::Init),
        Some("clone") => parse_clone(args),
        Some("compare") => parse_no_args(args, "compare", CliCommand::Compare),
        Some("sync") => parse_no_args(args, "sync", CliCommand::Sync),
        Some("resolve") => parse_no_args(args, "resolve", CliCommand::Resolve),
        Some("list") => {
            if is_help_request(&args[1..]) {
                Ok(CliCommand::Help(Some(OsString::from("list"))))
            } else {
                Ok(CliCommand::List(args.into_iter().skip(1).collect()))
            }
        }
        Some("clean") => {
            if is_help_request(&args[1..]) {
                Ok(CliCommand::Help(Some(OsString::from("clean"))))
            } else {
                Ok(CliCommand::Clean)
            }
        }
        _ => Ok(CliCommand::Git(args)),
    }
}

fn parse_help(args: Vec<OsString>) -> Result<CliCommand> {
    match args.as_slice() {
        [_] => Ok(CliCommand::Help(None)),
        [_, command] if is_native(command) => Ok(CliCommand::Help(Some(command.clone()))),
        [_, command] => Ok(CliCommand::GitHelp(command.clone())),
        _ => Err(MadmError::new("usage: madm help [command]")),
    }
}

fn parse_no_args(
    args: Vec<OsString>,
    command_name: &'static str,
    command: CliCommand,
) -> Result<CliCommand> {
    if is_help_request(&args[1..]) {
        return Ok(CliCommand::Help(Some(OsString::from(command_name))));
    }
    no_extra(args, command, &format!("madm {command_name}"))
}

fn parse_clone(args: Vec<OsString>) -> Result<CliCommand> {
    if is_help_request(&args[1..]) {
        return Ok(CliCommand::Help(Some(OsString::from("clone"))));
    }
    match args.as_slice() {
        [_, url] => Ok(CliCommand::Clone(url.clone())),
        _ => Err(MadmError::new("usage: madm clone <url>")),
    }
}

fn no_extra(args: Vec<OsString>, command: CliCommand, usage: &str) -> Result<CliCommand> {
    if args.len() == 1 {
        Ok(command)
    } else {
        Err(MadmError::new(format!("usage: {usage}")))
    }
}

fn is_help_request(args: &[OsString]) -> bool {
    matches!(args, [arg] if arg == OsStr::new("-h") || arg == OsStr::new("--help"))
}

fn is_native(command: &OsStr) -> bool {
    matches!(
        command.to_str(),
        Some(
            "init"
                | "clone"
                | "compare"
                | "sync"
                | "resolve"
                | "list"
                | "clean"
                | "help"
                | "version"
        )
    )
}

pub fn help(command: Option<&OsStr>) -> Result<&'static str> {
    match command.and_then(OsStr::to_str) {
        None => Ok(GENERAL_HELP),
        Some("init") => Ok(INIT_HELP),
        Some("clone") => Ok(CLONE_HELP),
        Some("compare") => Ok(COMPARE_HELP),
        Some("sync") => Ok(SYNC_HELP),
        Some("resolve") => Ok(RESOLVE_HELP),
        Some("list") => Ok(LIST_HELP),
        Some("clean") => Ok(CLEAN_HELP),
        Some("help") => Ok(HELP_HELP),
        Some("version") => Ok(VERSION_HELP),
        Some(other) => Err(MadmError::new(format!(
            "no native help is available for {other:?}"
        ))),
    }
}

const GENERAL_HELP: &str = "\
madm - a minimal, Git-native dotfiles manager

Usage:
  madm <command> [arguments...]
  madm <git-command-or-alias> [arguments...]

Native commands:
  init          Initialize an empty dotfiles repository
  clone <url>   Clone and safely reconcile an existing repository
  compare       Fetch and compare the effective home tree with upstream
  sync          Merge inbound changes, resolve conflicts, and push
  resolve       Resume madm's merge-conflict workflow
  list          Alias for git ls-files
  help          Show help for a madm or Git command

Any other command is passed to Git with:
  work tree:  <home>
  Git dir:   <home>/.local/share/madm/repo.git

Examples:
  madm init
  madm add .gitconfig
  madm commit -m \"Track Git configuration\"
  madm remote add origin <url>
  madm sync

Safety:
  madm clean is disabled because Git clean could delete untracked files
  throughout your home directory. Invoke raw Git deliberately if needed.

Run 'madm help <command>' for details.
";

const INIT_HELP: &str = "\
Usage: madm init

Create an empty Git repository at <home>/.local/share/madm/repo.git and use
the real home directory as its work tree. Existing paths are never replaced.
No home files are added, copied, moved, or committed.
";

const CLONE_HELP: &str = "\
Usage: madm clone <url>

Clone a remote repository without overwriting existing home files. Missing
tracked files are checked out. Differing files can be kept, replaced with the
remote version, or reviewed with the configured Git diff tool.

At each differing file:
  Enter   keep the local file untouched and unstaged
  a       keep all remaining local files
  r       replace this file with the remote version
  m       launch git difftool and leave the result unstaged
  q       stop resolving and keep all remaining local files

Without an interactive terminal, all differing local files are kept and
listed for later review.
";

const COMPARE_HELP: &str = "\
Usage: madm compare

Fetch the configured upstream and show:
  1. commit counts ahead of and behind upstream
  2. a full patch from upstream to the effective tracked home tree

The patch combines committed, staged, and unstaged tracked state. Compare
does not alter HEAD, the index, or work-tree files.
";

const SYNC_HELP: &str = "\
Usage: madm sync

Synchronize both directions: fetch, merge upstream without rebasing, resolve
merge conflicts, then push automatically. Tracked work-tree and index changes
must be committed first. Sync never auto-stashes, force-pushes, or rewrites
local commits.
";

const RESOLVE_HELP: &str = "\
Usage: madm resolve

Resume a conflicted Git merge. For each unresolved path, choose the local
side, upstream side, configured Git merge tool, skip, or quit. Enter skips
without staging anything. When every conflict is resolved, madm creates the
prepared merge commit automatically.

This command intentionally does not handle rebase or cherry-pick conflicts,
where Git's sides have different meanings.
";

const LIST_HELP: &str = "\
Usage: madm list [git-ls-files-options...]

Alias for 'git ls-files'. Every argument is forwarded unchanged.
";

const CLEAN_HELP: &str = "\
madm clean is disabled.

The home directory is madm's Git work tree. Commands such as 'git clean -fdx'
could therefore delete most of the user's files. Use raw Git with explicit
--git-dir and --work-tree arguments if this destructive operation is truly
intended.
";

const HELP_HELP: &str = "\
Usage: madm help [command]

Show madm help. For a non-native command, delegate to 'git help <command>'.
";

const VERSION_HELP: &str = "\
Usage: madm --version

Print the madm version.
";

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn unknown_commands_are_preserved_for_git() {
        let args = strings(&["status", "--short"]);
        assert_eq!(parse(args.clone()).unwrap(), CliCommand::Git(args));
    }

    #[test]
    fn list_forwards_arguments() {
        assert_eq!(
            parse(strings(&["list", "-m", "--", "a b"])).unwrap(),
            CliCommand::List(strings(&["-m", "--", "a b"]))
        );
    }

    #[test]
    fn native_help_is_recognized() {
        assert_eq!(
            parse(strings(&["clone", "--help"])).unwrap(),
            CliCommand::Help(Some(OsString::from("clone")))
        );
    }

    #[test]
    fn clone_requires_exactly_one_url() {
        assert!(parse(strings(&["clone"])).is_err());
        assert!(parse(strings(&["clone", "one", "two"])).is_err());
    }
}
