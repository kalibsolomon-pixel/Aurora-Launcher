//! Panic-safe boundary for asynchronous application calls from presentation.

use crate::api;

pub async fn guarded<F, T>(work: F) -> api::ApiResult<T>
where
    F: std::future::Future<Output = api::ApiResult<T>> + Send + 'static,
    T: Send + 'static,
{
    match tokio::spawn(work).await {
        Ok(result) => result,
        Err(_) => Err(api::ApiError::new(
            "native_operation_failed",
            "The operation stopped unexpectedly. Refresh the launcher state before retrying.",
        )),
    }
}
