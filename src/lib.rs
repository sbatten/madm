mod cli;
mod commands;
mod context;
mod error;
mod exclude;
mod git;
mod path;
mod prompt;
mod temp;
#[cfg(test)]
mod test_support;

use std::ffi::OsString;

use prompt::TerminalInteraction;

pub fn run(args: Vec<OsString>) -> i32 {
    let mut interaction = TerminalInteraction::new();
    match commands::execute(cli::parse(args), &mut interaction) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("madm: {}", error.message());
            error.code()
        }
    }
}
