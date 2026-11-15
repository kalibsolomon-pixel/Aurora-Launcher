//! Launcher-owned Pulsar recordings and immutable analytical views.
mod analytics;
mod cards;
mod parse;
mod projection;
mod reports;
mod service;
mod settings;
mod store;
pub use service::*;
pub mod view;
mod wire;

#[derive(Debug, Clone)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}
impl Error {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::new("pulsar_io", "The Pulsar file could not be accessed safely.")
    }
}

#[cfg(test)]
mod tests;
