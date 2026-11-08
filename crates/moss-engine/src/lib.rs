pub use moss_core::formula;
pub mod query;
pub use moss_core::rollup;
pub mod timeline;

pub use moss_core::EngineError;

pub(crate) fn error(message: impl Into<String>) -> EngineError {
    EngineError(message.into())
}
