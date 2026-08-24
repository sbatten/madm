use std::io::{self, IsTerminal, Write};

use crate::error::{MadmError, Result};

pub trait Interaction {
    fn is_interactive(&self) -> bool;
    fn prompt(&mut self, message: &str) -> Result<String>;
}

pub struct TerminalInteraction {
    interactive: bool,
}

impl TerminalInteraction {
    pub fn new() -> Self {
        Self {
            interactive: io::stdin().is_terminal() && io::stdout().is_terminal(),
        }
    }
}

impl Interaction for TerminalInteraction {
    fn is_interactive(&self) -> bool {
        self.interactive
    }

    fn prompt(&mut self, message: &str) -> Result<String> {
        print!("{message}");
        io::stdout()
            .flush()
            .map_err(|error| MadmError::io("write prompt", error))?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|error| MadmError::io("read prompt response", error))?;
        Ok(input.trim().to_owned())
    }
}

#[cfg(test)]
pub struct FakeInteraction {
    pub interactive: bool,
    pub responses: std::collections::VecDeque<String>,
    pub prompts: Vec<String>,
}

#[cfg(test)]
impl FakeInteraction {
    pub fn interactive(responses: &[&str]) -> Self {
        Self {
            interactive: true,
            responses: responses.iter().map(|value| (*value).to_owned()).collect(),
            prompts: Vec::new(),
        }
    }
}

#[cfg(test)]
impl Interaction for FakeInteraction {
    fn is_interactive(&self) -> bool {
        self.interactive
    }

    fn prompt(&mut self, message: &str) -> Result<String> {
        self.prompts.push(message.to_owned());
        self.responses
            .pop_front()
            .ok_or_else(|| MadmError::new("test prompt had no prepared response"))
    }
}
