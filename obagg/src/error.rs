use std::fmt;

#[derive(Debug)]
pub struct ObaggError(pub String);

impl fmt::Display for ObaggError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ObaggError {}
