//! Borealis is a static pre-blurred layer. Decorative motion must never
//! invalidate the whole software-rendered window during input or idle time.

pub fn sync() {}

/// Retained for opt-in startup acceptance markers; no background work is scheduled.
pub fn diagnostic(event: &str) {
    #[cfg(feature = "disposable-data-root")]
    if std::env::var_os("AURORA_DIAGNOSTIC_DATA_ROOT").is_some()
        && std::env::var_os("AURORA_DIAGNOSTIC_BOREALIS_TRACE").is_some()
    {
        eprintln!("[borealis] event={event} updates=0 static=true");
    }
    #[cfg(not(feature = "disposable-data-root"))]
    let _ = event;
}
