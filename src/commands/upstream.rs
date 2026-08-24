use std::ffi::{OsStr, OsString};

use crate::error::{MadmError, Result};
use crate::git::Git;

#[derive(Debug)]
pub struct Upstream {
    pub local_branch: String,
    pub remote: String,
    pub remote_branch: String,
    pub reference: String,
    pub inferred: bool,
    pub exists: bool,
}

pub fn resolve(git: &Git<'_>, allow_missing: bool) -> Result<Upstream> {
    let local_branch = current_branch(git)?;
    if let Some(mut upstream) = configured(git, &local_branch)? {
        fetch(git, &upstream.remote)?;
        pin_reference(git, &mut upstream)?;
        if !upstream.exists && !allow_missing {
            return Err(missing_upstream_error(&upstream));
        }
        return Ok(upstream);
    }

    let remotes = remotes(git)?;
    let remote = match remotes.as_slice() {
        [] => {
            return Err(MadmError::new(format!(
                "branch '{local_branch}' has no upstream and no remote is configured\n\
                 Add one and publish the branch with:\n  \
                 madm remote add origin <url>\n  \
                 madm push --set-upstream origin HEAD:{local_branch}"
            )));
        }
        [remote] => remote.clone(),
        _ => {
            let names = remotes
                .iter()
                .map(|remote| format!("  {remote}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(MadmError::new(format!(
                "branch '{local_branch}' has no upstream and multiple remotes are configured:\n\
                 {names}\n\
                 Configure the intended branch with:\n  \
                 madm branch --set-upstream-to=<remote>/{local_branch} {local_branch}"
            )));
        }
    };

    fetch(git, &remote)?;
    let reference = format!("refs/remotes/{remote}/{local_branch}");
    let mut upstream = Upstream {
        local_branch: local_branch.clone(),
        remote,
        remote_branch: local_branch,
        exists: false,
        reference,
        inferred: true,
    };
    pin_reference(git, &mut upstream)?;
    if !upstream.exists && !allow_missing {
        return Err(missing_upstream_error(&upstream));
    }
    Ok(upstream)
}

pub fn head_exists(git: &Git<'_>) -> Result<bool> {
    revision_exists(git, "HEAD")
}

fn current_branch(git: &Git<'_>) -> Result<String> {
    let output = git.output([
        OsStr::new("symbolic-ref"),
        OsStr::new("--quiet"),
        OsStr::new("--short"),
        OsStr::new("HEAD"),
    ])?;
    if !output.status.success() {
        return Err(MadmError::new(
            "HEAD is detached; check out a local branch before using this command",
        ));
    }
    String::from_utf8(output.stdout)
        .map(|branch| branch.trim().to_owned())
        .map_err(|_| MadmError::new("the current Git branch name is not valid UTF-8"))
}

fn configured(git: &Git<'_>, local_branch: &str) -> Result<Option<Upstream>> {
    let remote_key = format!("branch.{local_branch}.remote");
    let merge_key = format!("branch.{local_branch}.merge");
    let remote = config_value(git, &remote_key)?;
    let merge = config_value(git, &merge_key)?;

    match (remote, merge) {
        (None, None) => Ok(None),
        (Some(remote), Some(merge)) => {
            let remote_branch = merge
                .strip_prefix("refs/heads/")
                .ok_or_else(|| {
                    MadmError::new(format!(
                        "upstream merge ref for '{local_branch}' is not a branch: {merge}"
                    ))
                })?
                .to_owned();
            let reference = if remote == "." {
                merge.clone()
            } else {
                String::from("@{upstream}")
            };
            Ok(Some(Upstream {
                local_branch: local_branch.to_owned(),
                remote,
                remote_branch,
                reference,
                inferred: false,
                exists: false,
            }))
        }
        _ => Err(MadmError::new(format!(
            "branch '{local_branch}' has incomplete upstream configuration; set it with:\n  \
             madm branch --set-upstream-to=<remote>/<branch> {local_branch}"
        ))),
    }
}

fn config_value(git: &Git<'_>, key: &str) -> Result<Option<String>> {
    let output = git.output([OsStr::new("config"), OsStr::new("--get"), OsStr::new(key)])?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map(|value| Some(value.trim().to_owned()))
            .map_err(|_| MadmError::new(format!("Git config value '{key}' is not valid UTF-8")))
    } else if output.status.code() == Some(1) {
        Ok(None)
    } else {
        Err(crate::git::output_error(
            &format!("read Git config value '{key}'"),
            output,
        ))
    }
}

fn remotes(git: &Git<'_>) -> Result<Vec<String>> {
    let output = git.text([OsStr::new("remote")], "list Git remotes")?;
    if output.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(output.lines().map(str::to_owned).collect())
    }
}

fn fetch(git: &Git<'_>, remote: &str) -> Result<()> {
    if remote == "." {
        return Ok(());
    }
    git.checked_status(
        [OsStr::new("fetch"), OsStr::new("--"), OsStr::new(remote)],
        &format!("fetch remote '{remote}'"),
    )
}

fn revision_exists(git: &Git<'_>, revision: &str) -> Result<bool> {
    Ok(resolve_revision(git, revision)?.is_some())
}

fn pin_reference(git: &Git<'_>, upstream: &mut Upstream) -> Result<()> {
    if let Some(commit) = resolve_revision(git, &upstream.reference)? {
        upstream.reference = commit;
        upstream.exists = true;
    } else {
        upstream.exists = false;
    }
    Ok(())
}

fn resolve_revision(git: &Git<'_>, revision: &str) -> Result<Option<String>> {
    let mut commit = OsString::from(revision);
    commit.push("^{commit}");
    let output = git.output([
        OsStr::new("rev-parse"),
        OsStr::new("--verify"),
        OsStr::new("--quiet"),
        commit.as_os_str(),
    ])?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map(|value| Some(value.trim().to_owned()))
            .map_err(|_| {
                MadmError::new(format!(
                    "resolved Git revision '{revision}' is not valid UTF-8"
                ))
            })
    } else if matches!(output.status.code(), Some(1 | 128)) {
        Ok(None)
    } else {
        Err(crate::git::output_error(
            &format!("resolve Git revision '{revision}'"),
            output,
        ))
    }
}

fn missing_upstream_error(upstream: &Upstream) -> MadmError {
    MadmError::new(format!(
        "remote branch '{}/{}' does not exist\n\
         Publish it and set upstream with:\n  \
         madm push --set-upstream {} HEAD:{}",
        upstream.remote, upstream.remote_branch, upstream.remote, upstream.remote_branch
    ))
}
