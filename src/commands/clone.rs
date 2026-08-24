use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::PathBuf;

use crate::context::Context;
use crate::error::{MadmError, Result};
use crate::git::{self, Git};
use crate::path::{existing_path_blocker, nul_paths};
use crate::prompt::Interaction;
use crate::temp::TemporaryDirectory;

use super::{
    cleanup_failed_setup, configure_new_repository, ensure_reserved_path_is_untracked,
    ensure_revision_avoids_reserved_path,
};

pub fn run(url: OsString, interaction: &mut dyn Interaction) -> Result<i32> {
    let context = Context::discover()?;
    refuse_existing_target(&context)?;
    let temporary = TemporaryDirectory::create("madm-clone")?;
    let created_parents = context.create_repository_parent()?;
    let checkout = temporary.path().join("checkout");
    let mut separate_git_dir = OsString::from("--separate-git-dir=");
    separate_git_dir.push(context.repository().as_os_str());
    let clone_args = vec![
        OsString::from("clone"),
        OsString::from("--no-checkout"),
        separate_git_dir,
        url,
        checkout.as_os_str().to_owned(),
    ];

    let status = match git::raw_status(clone_args) {
        Ok(status) => status,
        Err(error) => return cleanup_failed_setup(&context, &created_parents, error),
    };
    if !status.success() {
        return cleanup_failed_setup(
            &context,
            &created_parents,
            MadmError::with_code(
                "Git could not clone the madm repository",
                git::status_code(status),
            ),
        );
    }

    if let Err(error) = prepare_cloned_repository(&context) {
        return cleanup_failed_setup(&context, &created_parents, error);
    }

    if let Err(error) = temporary.remove() {
        return cleanup_failed_setup(&context, &created_parents, error);
    }

    let git = Git::new(&context);
    let conflicts = checkout_missing_files(&context, &git)?;
    let modified = modified_paths(&git)?;
    let paths = merge_unique_paths(conflicts, modified);

    println!(
        "Cloned madm repository into {}",
        context.repository().display()
    );
    reconcile_existing_files(&git, &paths, interaction)?;
    Ok(0)
}

fn refuse_existing_target(context: &Context) -> Result<()> {
    match fs::symlink_metadata(context.repository()) {
        Ok(_) => Err(MadmError::new(format!(
            "refusing to overwrite existing repository path: {}",
            context.repository().display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(MadmError::io("inspect repository path", error)),
    }
}

fn prepare_cloned_repository(context: &Context) -> Result<()> {
    configure_new_repository(context)?;
    let git = Git::new(context);
    let head = git.output([
        OsStr::new("rev-parse"),
        OsStr::new("--verify"),
        OsStr::new("--quiet"),
        OsStr::new("HEAD"),
    ])?;
    if head.status.success() {
        ensure_revision_avoids_reserved_path(&git, "HEAD")?;
        git.checked_status(
            [
                OsStr::new("reset"),
                OsStr::new("--quiet"),
                OsStr::new("--"),
                OsStr::new(":/"),
            ],
            "populate the cloned repository index",
        )?;
        ensure_reserved_path_is_untracked(&git)?;
    }
    Ok(())
}

fn checkout_missing_files(context: &Context, git: &Git<'_>) -> Result<Vec<PathBuf>> {
    let missing = nul_paths(git.checked_output(
        [
            OsStr::new("ls-files"),
            OsStr::new("--deleted"),
            OsStr::new("-z"),
        ],
        "list missing cloned files",
    )?)?;
    let mut conflicts = Vec::new();

    for path in missing {
        if existing_path_blocker(context.home(), &path)?.is_some() {
            conflicts.push(path);
            continue;
        }
        let status = git.status([OsStr::new("checkout"), OsStr::new("--"), path.as_os_str()])?;
        if !status.success() {
            eprintln!(
                "madm: could not safely check out {}; the local path was left untouched",
                path.display()
            );
            conflicts.push(path);
        }
    }

    Ok(conflicts)
}

fn modified_paths(git: &Git<'_>) -> Result<Vec<PathBuf>> {
    nul_paths(git.checked_output(
        [
            OsStr::new("ls-files"),
            OsStr::new("--modified"),
            OsStr::new("-z"),
        ],
        "list differing cloned files",
    )?)
}

fn merge_unique_paths(first: Vec<PathBuf>, second: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    first
        .into_iter()
        .chain(second)
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn reconcile_existing_files(
    git: &Git<'_>,
    paths: &[PathBuf],
    interaction: &mut dyn Interaction,
) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    if !interaction.is_interactive() {
        println!("Kept differing local files untouched and unstaged:");
        for path in paths {
            println!("  {}", path.display());
        }
        println!("Run 'madm diff' or 'madm compare' to review them.");
        return Ok(());
    }

    for (index, path) in paths.iter().enumerate() {
        loop {
            let response = interaction.prompt(&format!(
                "{} differs [Enter=keep local, a=keep all, r=remote, m=difftool, q=quit]: ",
                path.display()
            ))?;
            match response.to_ascii_lowercase().as_str() {
                "" => {
                    println!("Kept {} untouched and unstaged.", path.display());
                    break;
                }
                "a" => {
                    print_remaining_local(paths, index);
                    return Ok(());
                }
                "q" => {
                    print_remaining_local(paths, index);
                    return Ok(());
                }
                "r" => {
                    git.checked_status(
                        [OsStr::new("checkout"), OsStr::new("--"), path.as_os_str()],
                        &format!("replace {} with the remote version", path.display()),
                    )?;
                    println!("Replaced {} with the remote version.", path.display());
                    break;
                }
                "m" => {
                    let status = git.status([
                        OsStr::new("difftool"),
                        OsStr::new("--no-prompt"),
                        OsStr::new("--"),
                        path.as_os_str(),
                    ])?;
                    if status.success() {
                        println!("Left the difftool result for {} unstaged.", path.display());
                        break;
                    }
                    eprintln!(
                        "madm: the configured Git diff tool failed for {}; choose another action",
                        path.display()
                    );
                }
                _ => eprintln!("madm: choose Enter, a, r, m, or q"),
            }
        }
    }
    Ok(())
}

fn print_remaining_local(paths: &[PathBuf], current: usize) {
    println!("Kept remaining local files untouched and unstaged:");
    for path in &paths[current..] {
        println!("  {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::FakeInteraction;
    use crate::test_support::GitFixture;

    #[test]
    fn keeping_a_clone_conflict_does_not_stage_it() {
        let fixture = GitFixture::new();
        fixture.write(".config", "remote");
        fixture.commit_all("base");
        fixture.write(".config", "local");
        let git = fixture.git();
        let mut interaction = FakeInteraction::interactive(&[""]);

        reconcile_existing_files(&git, &[PathBuf::from(".config")], &mut interaction).unwrap();

        assert_eq!(fixture.read(".config"), "local");
        assert_eq!(
            git.text(
                [
                    OsStr::new("diff"),
                    OsStr::new("--cached"),
                    OsStr::new("--name-only"),
                ],
                "inspect index",
            )
            .unwrap(),
            ""
        );
    }

    #[test]
    fn accepting_remote_replaces_only_the_selected_path() {
        let fixture = GitFixture::new();
        fixture.write(".config", "remote");
        fixture.commit_all("base");
        fixture.write(".config", "local");
        let git = fixture.git();
        let mut interaction = FakeInteraction::interactive(&["r"]);

        reconcile_existing_files(&git, &[PathBuf::from(".config")], &mut interaction).unwrap();

        assert_eq!(fixture.read(".config"), "remote");
        assert_eq!(
            git.text(
                [OsStr::new("status"), OsStr::new("--porcelain=v1")],
                "inspect work tree",
            )
            .unwrap(),
            ""
        );
    }
}
