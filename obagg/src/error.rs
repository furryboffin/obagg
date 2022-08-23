#[derive(Debug)]
pub enum Error {
    Boxed(Box<dyn std::error::Error + Sync + Send>),
    Tonic(tonic::transport::Error),
    Tungstenite(tokio_tungstenite::tungstenite::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Boxed(error) => write!(f, "{}", error),
            Self::Tonic(error) => write!(f, "{}", error),
            Self::Tungstenite(error) => write!(f, "{}", error),

        }
    }
}

impl std::error::Error for Error {}

impl From<Box<dyn std::error::Error + Sync + Send>> for Error {
    fn from(error: Box<dyn std::error::Error + Sync + Send>) -> Self {
        Error::Boxed(error)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::Tungstenite(error)
    }
}
