use std::fmt::{self, Display, Formatter};
use std::io;

pub type Result<T> = std::result::Result<T, MadmError>;

#[derive(Debug)]
pub struct MadmError {
    message: String,
    code: i32,
}

impl MadmError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 1,
        }
    }

    pub fn with_code(message: impl Into<String>, code: i32) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }

    pub fn io(action: &str, error: io::Error) -> Self {
        Self::new(format!("{action}: {error}"))
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn code(&self) -> i32 {
        self.code
    }
}

impl Display for MadmError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MadmError {}
