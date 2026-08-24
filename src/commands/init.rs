use std::ffi::OsString;

use crate::context::Context;
use crate::error::{MadmError, Result};
use crate::git;

use super::{cleanup_failed_setup, configure_new_repository};

pub fn run() -> Result<i32> {
    let context = Context::discover()?;
    match std::fs::symlink_metadata(context.repository()) {
        Ok(_) => {
            return Err(MadmError::new(format!(
                "refusing to overwrite existing repository path: {}",
                context.repository().display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(MadmError::io("inspect repository path", error)),
    }

    let created_parents = context.create_repository_parent()?;

    let args = vec![
        OsString::from("init"),
        OsString::from("--bare"),
        context.repository().as_os_str().to_owned(),
    ];
    let status = match git::raw_status(args) {
        Ok(status) => status,
        Err(error) => return cleanup_failed_setup(&context, &created_parents, error),
    };
    if !status.success() {
        let error = MadmError::with_code(
            "Git could not initialize the madm repository",
            git::status_code(status),
        );
        return cleanup_failed_setup(&context, &created_parents, error);
    }

    if let Err(error) = configure_new_repository(&context) {
        return cleanup_failed_setup(&context, &created_parents, error);
    }

    println!(
        "Initialized madm repository at {}",
        context.repository().display()
    );
    Ok(0)
}
