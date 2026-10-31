//! One 2 Hz timer drifts the pre-blurred Borealis layer while active.
//! Pausing freezes phase; resuming never catches up suspended work.
use crate::{Theme, ui};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(500);
const AMPLITUDE_PX: f32 = 12.0;
const PERIOD_S: f32 = 90.0;

#[derive(Default)]
struct MotionClock {
    elapsed: Duration,
    previous: Option<Instant>,
    updates: u64,
}
impl MotionClock {
    fn running(&mut self, run: bool, now: Instant) {
        self.previous = run.then_some(now);
    }
    fn advance(&mut self, now: Instant) -> f32 {
        if let Some(previous) = self.previous.replace(now) {
            // A delayed callback advances at most one step, never a burst.
            self.elapsed += now.saturating_duration_since(previous).min(TICK);
        }
        self.updates += 1;
        AMPLITUDE_PX * (self.elapsed.as_secs_f32() / PERIOD_S * std::f32::consts::TAU).sin()
    }
}
thread_local! {
    static TIMER: RefCell<Option<slint::Timer>> = const { RefCell::new(None) };
    static CLOCK: RefCell<MotionClock> = RefCell::default();
    static ORIGIN: Instant = Instant::now();
}

fn allowed(borealis: bool, reduced: bool, visible: bool, foreground: bool, forced: bool) -> bool {
    borealis && !reduced && visible && (foreground || forced)
}
fn should_run() -> bool {
    ui::with(|window, _| {
        let theme = window.global::<Theme>();
        allowed(
            theme.get_borealis(),
            theme.get_reduce_motion(),
            crate::chrome::is_visible(),
            crate::chrome::is_foreground(),
            measuring_active_motion(),
        )
    })
    .unwrap_or(false)
}

/// Reuse the same timer, including when stopped from inside its callback.
pub fn sync() {
    let run = should_run();
    TIMER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let was_running = slot.as_ref().is_some_and(slint::Timer::running);
        if run == was_running {
            return;
        }
        CLOCK.with(|clock| clock.borrow_mut().running(run, Instant::now()));
        if run {
            if let Some(timer) = slot.as_ref() {
                timer.restart();
            } else {
                let timer = slint::Timer::default();
                timer.start(slint::TimerMode::Repeated, TICK, tick);
                *slot = Some(timer);
            }
        } else if let Some(timer) = slot.as_ref() {
            timer.stop();
        }
        diagnostic(if run { "running" } else { "paused" });
    });
}

#[cfg(feature = "disposable-data-root")]
fn diagnostic_root() -> bool {
    std::env::var_os("AURORA_DIAGNOSTIC_DATA_ROOT")
        .is_some_and(|root| std::path::Path::new(&root).is_absolute())
}
fn measuring_active_motion() -> bool {
    #[cfg(feature = "disposable-data-root")]
    {
        diagnostic_root() && std::env::var_os("AURORA_DIAGNOSTIC_BOREALIS_ACTIVE").is_some()
    }
    #[cfg(not(feature = "disposable-data-root"))]
    {
        false
    }
}

/// Numeric acceptance telemetry only; no paths, account data or UI text.
pub fn diagnostic(event: &str) {
    #[cfg(feature = "disposable-data-root")]
    if diagnostic_root() && std::env::var_os("AURORA_DIAGNOSTIC_BOREALIS_TRACE").is_some() {
        CLOCK.with(|clock| {
            let clock = clock.borrow();
            eprintln!("[borealis] {{\"event\":\"{}\",\"ms\":{},\"updates\":{},\"phaseMs\":{},\"visible\":{},\"foreground\":{}}}",
                event, ORIGIN.with(|origin| origin.elapsed().as_millis()), clock.updates,
                clock.elapsed.as_millis(), crate::chrome::is_visible(), crate::chrome::is_foreground());
        });
    }
    #[cfg(not(feature = "disposable-data-root"))]
    let _ = event;
}
fn tick() {
    if !should_run() {
        sync();
        return;
    }
    let offset = CLOCK.with(|clock| clock.borrow_mut().advance(Instant::now()));
    ui::with(|window, _| window.global::<Theme>().set_drift(offset));
    diagnostic("tick");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreground_override_never_bypasses_visibility_motion_or_theme() {
        for forced in [false, true] {
            for foreground in [false, true] {
                assert!(!allowed(true, false, false, foreground, forced));
                assert!(!allowed(true, true, true, foreground, forced));
                assert!(!allowed(false, false, true, foreground, forced));
                assert_eq!(
                    allowed(true, false, true, foreground, forced),
                    foreground || forced
                );
            }
        }
    }
    #[test]
    fn suspension_freezes_phase_and_delays_never_accumulate_work() {
        let start = Instant::now();
        let mut clock = MotionClock::default();
        clock.running(true, start);
        let first = clock.advance(start + TICK);
        clock.running(false, start + TICK);
        assert_eq!(clock.elapsed, TICK);
        clock.running(true, start + Duration::from_secs(60));
        let second = clock.advance(start + Duration::from_secs(60) + TICK);
        assert_eq!(clock.elapsed, TICK * 2);
        assert!(second > first);
        clock.advance(start + Duration::from_secs(120));
        assert_eq!(clock.elapsed, TICK * 3);
        assert_eq!(clock.updates, 3);
    }
}
