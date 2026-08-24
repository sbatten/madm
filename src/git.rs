use std::ffi::{OsStr, OsString};
use std::io;
use std::process::{Command, ExitStatus, Output, Stdio};

use crate::context::Context;
use crate::error::{MadmError, Result};

pub struct Git<'a> {
    context: &'a Context,
}

impl<'a> Git<'a> {
    pub fn new(context: &'a Context) -> Self {
        Self { context }
    }

    pub fn status<I, S>(&self, args: I) -> Result<ExitStatus>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.command();
        command.args(args);
        spawn_status(command)
    }

    pub fn checked_status<I, S>(&self, args: I, action: &str) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let status = self.status(args)?;
        if status.success() {
            Ok(())
        } else {
            Err(MadmError::with_code(
                format!("{action} failed with {}", describe_status(status)),
                status_code(status),
            ))
        }
    }

    pub fn output<I, S>(&self, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.command();
        command.args(args).env("LC_ALL", "C").stdin(Stdio::null());
        spawn_output(command)
    }

    pub fn checked_output<I, S>(&self, args: I, action: &str) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.output(args)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(output_error(action, output))
        }
    }

    pub fn text<I, S>(&self, args: I, action: &str) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let bytes = self.checked_output(args, action)?;
        String::from_utf8(bytes)
            .map(|text| text.trim().to_owned())
            .map_err(|_| MadmError::new(format!("{action}: Git returned non-UTF-8 text")))
    }

    pub fn passthrough(&self, args: &[OsString]) -> Result<i32> {
        let status = self.status(args)?;
        Ok(status_code(status))
    }

    fn command(&self) -> Command {
        contextual_command(self.context)
    }
}

pub fn raw_status<I, S>(args: I) -> Result<ExitStatus>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    command
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE");
    spawn_status(command)
}

pub fn status_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

pub fn output_error(action: &str, output: Output) -> MadmError {
    let detail = String::from_utf8_lossy(&output.stderr);
    let detail = detail.trim();
    let message = if detail.is_empty() {
        format!("{action} failed with {}", describe_status(output.status))
    } else {
        format!("{action} failed: {detail}")
    };
    MadmError::with_code(message, status_code(output.status))
}

fn contextual_command(context: &Context) -> Command {
    let mut git_dir = OsString::from("--git-dir=");
    git_dir.push(context.repository().as_os_str());
    let mut work_tree = OsString::from("--work-tree=");
    work_tree.push(context.home().as_os_str());

    let mut command = Command::new("git");
    command
        .arg(git_dir)
        .arg(work_tree)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE");
    command
}

fn spawn_status(mut command: Command) -> Result<ExitStatus> {
    command
        .status()
        .map_err(|error| process_error("run Git", error))
}

fn spawn_output(mut command: Command) -> Result<Output> {
    command
        .output()
        .map_err(|error| process_error("run Git", error))
}

fn process_error(action: &str, error: io::Error) -> MadmError {
    if error.kind() == io::ErrorKind::NotFound {
        MadmError::new("Git is required, but 'git' was not found on PATH")
    } else {
        MadmError::io(action, error)
    }
}

fn describe_status(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit code {code}"),
        None => "termination by signal".to_owned(),
    }
}
