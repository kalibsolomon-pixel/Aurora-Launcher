//! No desktop interaction: refuse a missing/foreign owner before opening COM
//! UI, and prove a refused request releases the shared single-picker guard.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "../src/api.rs"]
mod api;
#[allow(dead_code)]
#[path = "../src/skin_import.rs"]
mod skin_import;

#[tokio::test]
async fn custom_java_picker_requires_a_live_owned_window_and_remains_retryable() {
    for owner in [None, Some(0), None] {
        let error = skin_import::choose_java_path(owner).await.unwrap_err();
        assert_eq!(error.code, "skin_import_failed");
    }
}
