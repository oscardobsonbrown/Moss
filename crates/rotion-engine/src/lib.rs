pub mod formula;
pub mod query;
pub mod rollup;
pub mod timeline;

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("{0}")]
pub struct EngineError(pub String);

pub(crate) fn error(message: impl Into<String>) -> EngineError {
    EngineError(message.into())
}
