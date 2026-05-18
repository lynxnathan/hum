use thiserror::Error;

#[derive(Debug, Error)]
pub enum HumParseError {
    #[error("parse error in .hum file: {0}")]
    InvalidSchema(String),

    #[error("IO error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
