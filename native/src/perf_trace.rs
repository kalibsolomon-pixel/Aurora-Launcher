//! Opt-in frame and navigation timing for local performance measurements.
//! Only debug-assertion builds started with AURORA_DIAGNOSTIC_PERF_TRACE=1
//! write `[perf]` lines to stderr; every other build compiles this to nothing.

#[cfg(debug_assertions)]
mod enabled {
    use std::cell::Cell;
    use std::sync::OnceLock;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    thread_local! {
        static FRAME_START: Cell<Option<(u128, Instant)>> = const { Cell::new(None) };
    }

    fn active() -> bool {
        static ACTIVE: OnceLock<bool> = OnceLock::new();
        *ACTIVE.get_or_init(|| {
            std::env::var_os("AURORA_DIAGNOSTIC_PERF_TRACE").as_deref()
                == Some(std::ffi::OsStr::new("1"))
        })
    }

    fn now_us() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|time| time.as_micros())
            .unwrap_or_default()
    }

    pub fn mark(event: &str, detail: impl std::fmt::Display) {
        if active() {
            eprintln!("[perf] {} {event} {detail}", now_us());
        }
    }

    pub fn frame_begin() {
        if active() {
            FRAME_START.with(|start| start.set(Some((now_us(), Instant::now()))));
        }
    }

    pub fn frame_end() {
        if let Some((at, start)) = FRAME_START.with(Cell::take) {
            eprintln!("[perf] {at} frame {}", start.elapsed().as_micros());
        }
    }
}

#[cfg(debug_assertions)]
pub use enabled::{frame_begin, frame_end, mark};

#[cfg(not(debug_assertions))]
pub fn mark(_event: &str, _detail: impl std::fmt::Display) {}
#[cfg(not(debug_assertions))]
pub fn frame_begin() {}
#[cfg(not(debug_assertions))]
pub fn frame_end() {}
