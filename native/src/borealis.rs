//! Bounded motion for the cached, pre-blurred Borealis band layers. Software
//! rendering remains static; accelerated rendering uses one stoppable phase
//! source whose cadence, speed and reach follow the Motion setting.

use std::cell::RefCell;
use std::f32::consts::TAU;
use std::time::Duration;

use slint::ComponentHandle;

use crate::{AppWindow, Borealis, BorealisPose, Settings, Theme, ui};

struct Motion {
    timer: slint::Timer,
    interval: Duration,
    phase: f32,
    accelerated: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            timer: slint::Timer::default(),
            interval: Duration::ZERO,
            phase: 0.0,
            accelerated: false,
        }
    }
}

thread_local! {
    static MOTION: RefCell<Motion> = RefCell::new(Motion::default());
}

/// One sinusoid of a band pose: phase multiplier, phase offset, reach.
type Wave = (f32, f32, f32);

/// Every multiplier has two decimals, so all waves repeat after 100 turns;
/// wrapping there keeps the phase precise without a visible jump.
const PERIOD: f32 = 100.0 * TAU;

/// Per band: horizontal and vertical drift (window fractions), turn (degrees),
/// horizontal and vertical stretch. Different multipliers keep bands crossing.
const WAVES: [[Wave; 5]; 4] = [
    [
        (0.83, 0.0, 0.10),
        (0.61, 1.3, 0.07),
        (0.47, 2.1, 5.0),
        (0.71, 0.4, 0.04),
        (0.97, 2.7, 0.16),
    ],
    [
        (0.67, 2.4, 0.11),
        (0.91, 0.2, 0.06),
        (0.59, 4.0, 5.0),
        (0.53, 1.9, 0.05),
        (0.79, 0.9, 0.18),
    ],
    [
        (0.51, 4.2, 0.09),
        (0.73, 3.1, 0.05),
        (0.37, 0.7, 4.0),
        (0.89, 3.3, 0.04),
        (0.63, 4.4, 0.14),
    ],
    [
        (1.07, 1.1, 0.11),
        (0.83, 5.0, 0.08),
        (0.71, 3.3, 6.0),
        (0.61, 5.5, 0.05),
        (1.13, 1.6, 0.20),
    ],
];

/// Installs actual-renderer detection. A software renderer does not provide
/// Slint's rendering notifier, so its unsupported result selects the static path.
pub fn install(window: &AppWindow) {
    let weak = window.as_weak();
    let result = window
        .window()
        .set_rendering_notifier(move |state, api| match state {
            slint::RenderingState::RenderingSetup => {
                let accelerated = matches!(api, slint::GraphicsAPI::NativeOpenGL { .. });
                MOTION.with(|slot| slot.borrow_mut().accelerated = accelerated);
                if let Some(window) = weak.upgrade() {
                    window.global::<Borealis>().set_accelerated(accelerated);
                    window
                        .global::<Settings>()
                        .set_renderer(format!("Slint renderer · actual {api:?}").into());
                }
                sync();
            }
            slint::RenderingState::RenderingTeardown => {
                // Slint tears the renderer down (on close) while it holds the
                // window adapter borrowed. A property write here requests a
                // redraw re-entrantly and panics, leaving the GL context alive
                // into process exit, where the driver never finishes. Stop the
                // motion now; republish the static pose once teardown returns.
                MOTION.with(|slot| {
                    let mut motion = slot.borrow_mut();
                    motion.accelerated = false;
                    motion.timer.stop();
                });
                let weak = weak.clone();
                slint::Timer::single_shot(Duration::ZERO, move || {
                    if let Some(window) = weak.upgrade() {
                        window.global::<Borealis>().set_accelerated(false);
                    }
                    sync();
                });
            }
            slint::RenderingState::BeforeRendering => crate::perf_trace::frame_begin(),
            slint::RenderingState::AfterRendering => crate::perf_trace::frame_end(),
            _ => {}
        });
    if let Err(error) = result {
        MOTION.with(|slot| slot.borrow_mut().accelerated = false);
        ui::with(|window, _| {
            window.global::<Borealis>().set_accelerated(false);
            window.global::<Settings>().set_renderer(
                format!("Slint software renderer · rendering notifier {error:?}").into(),
            );
        });
        sync();
    }
}

fn requested() -> (u8, bool) {
    ui::with(|window, _| {
        (
            window
                .global::<Settings>()
                .get_motion_speed()
                .round()
                .clamp(0.0, 100.0) as u8,
            window.global::<Theme>().get_borealis(),
        )
    })
    .unwrap_or((0, false))
}

fn accelerated() -> bool {
    MOTION.with(|slot| slot.borrow().accelerated)
}

/// Resynchronizes after appearance, renderer and native visibility events.
/// Hidden accelerated windows retain their phase but never catch up on resume.
pub fn sync() {
    let (speed, borealis) = requested();
    let accelerated = accelerated();
    let run = accelerated && borealis && speed > 0 && crate::chrome::is_visible();

    let reset = speed == 0 || !borealis || !accelerated;
    let interval = cadence(speed);
    let phase = MOTION.with(|slot| {
        let mut motion = slot.borrow_mut();
        if !run || motion.interval != interval {
            motion.timer.stop();
        }
        if reset {
            motion.phase = 0.0;
        }
        if run && !motion.timer.running() {
            motion.interval = interval;
            motion
                .timer
                .start(slint::TimerMode::Repeated, interval, tick);
        }
        motion.phase
    });
    // Also republish after speed changes, without advancing a paused phase.
    publish(phase, speed);
}

fn tick() {
    let (speed, borealis) = requested();
    let interval = MOTION.with(|slot| slot.borrow().interval);
    if !accelerated()
        || !borealis
        || speed == 0
        || !crate::chrome::is_visible()
        || interval != cadence(speed)
    {
        sync();
        return;
    }

    let phase = MOTION.with(|slot| {
        let mut motion = slot.borrow_mut();
        // Fixed phase steps: delayed frames and hidden windows never catch up.
        motion.phase = advance(motion.phase, speed);
        motion.phase
    });
    publish(phase, speed);
}

fn publish(phase: f32, speed: u8) {
    ui::with(|window, _| {
        let layer = window.global::<Borealis>();
        let poses = [0, 1, 2, 3].map(|band| pose(band, phase, speed));
        if layer.get_band_0() != poses[0] {
            layer.set_band_0(poses[0].clone());
        }
        if layer.get_band_1() != poses[1] {
            layer.set_band_1(poses[1].clone());
        }
        if layer.get_band_2() != poses[2] {
            layer.set_band_2(poses[2].clone());
        }
        if layer.get_band_3() != poses[3] {
            layer.set_band_3(poses[3].clone());
        }
    });
}

/// Timer period: 10 Hz at the lowest nonzero speed, 24 Hz at 100. The bands
/// are heavily blurred, so the coarser steps of slow speeds stay soft.
fn cadence(speed: u8) -> Duration {
    let hertz = 10.0 + 14.0 * f32::from(speed) / 100.0;
    Duration::from_millis((1000.0 / hertz).round() as u64)
}

/// Phase velocity in radians per second: about 0.29 at 50, 0.70 at 100.
fn velocity(speed: u8) -> f32 {
    let speed = f32::from(speed) / 100.0;
    0.06 + 0.64 * speed * speed.sqrt()
}

/// Share of each band's full reach: about 0.73 at 50, all of it at 100.
fn reach(speed: u8) -> f32 {
    0.45 + 0.55 * f32::from(speed) / 100.0
}

fn advance(phase: f32, speed: u8) -> f32 {
    (phase + velocity(speed) * cadence(speed).as_secs_f32()) % PERIOD
}

/// Each wave is measured from phase 0, so phase 0 (and motion 0) is exactly
/// the all-zero rest pose and resets never jump.
fn pose(band: usize, phase: f32, speed: u8) -> BorealisPose {
    if speed == 0 {
        return BorealisPose::default();
    }
    let reach = reach(speed);
    let wave = |(multiplier, offset, extent): Wave| {
        reach * extent * ((phase * multiplier + offset).sin() - offset.sin())
    };
    let [dx, dy, turn, grow_x, grow_y] = WAVES[band].map(wave);
    BorealisPose {
        dx,
        dy,
        turn,
        grow_x,
        grow_y,
    }
}

/// Retained for opt-in startup acceptance markers; no background work is scheduled.
pub fn diagnostic(event: &str) {
    #[cfg(feature = "disposable-data-root")]
    if std::env::var_os("AURORA_DIAGNOSTIC_DATA_ROOT").is_some()
        && std::env::var_os("AURORA_DIAGNOSTIC_BOREALIS_TRACE").is_some()
    {
        MOTION.with(|slot| {
            let motion = slot.borrow();
            eprintln!(
                "[borealis] event={event} accelerated={} timer_running={} phase={}",
                motion.accelerated,
                motion.timer.running(),
                motion.phase
            );
        });
    }
    #[cfg(not(feature = "disposable-data-root"))]
    let _ = event;
}

#[cfg(test)]
mod tests {
    use super::{PERIOD, TAU, WAVES, advance, cadence, pose};
    use std::time::Duration;

    /// Rest placement mirrored from the BorealisBackdrop in borealis.slint: centre
    /// x/y as window fractions, width as window-width fraction (plus 0.4 of
    /// the window height), height as window-height fraction, rest turn.
    const PLACEMENT: [(f32, f32, f32, f32, f32); 4] = [
        (0.42, 0.66, 1.70, 0.78, -14.0),
        (0.72, 0.14, 1.85, 0.70, 10.0),
        (0.58, 0.90, 1.65, 0.70, -6.0),
        (0.48, 0.42, 1.60, 0.52, 10.0),
    ];

    #[test]
    fn motion_zero_and_phase_zero_are_the_rest_pose() {
        for band in 0..4 {
            assert_eq!(pose(band, 1.3, 0), Default::default());
            for speed in [1, 50, 100] {
                let rest = pose(band, 0.0, speed);
                assert!(rest.dx.abs() + rest.dy.abs() + rest.turn.abs() < 1e-6);
                assert!(rest.grow_x.abs() + rest.grow_y.abs() < 1e-6);
            }
        }
    }

    #[test]
    fn phase_wrap_does_not_jump() {
        for waves in WAVES {
            for (multiplier, _, _) in waves {
                let hundredths = multiplier * 100.0;
                assert!((hundredths - hundredths.round()).abs() < 1e-4);
            }
        }
        for band in 0..4 {
            let before = pose(band, PERIOD - 0.01, 100);
            let after = pose(band, advance(PERIOD - 0.01, 100), 100);
            assert!((before.dx - after.dx).abs() < 0.01);
            assert!((before.turn - after.turn).abs() < 0.5);
        }
    }

    #[test]
    fn higher_speed_moves_further_per_second() {
        let travel = |speed: u8| {
            let ticks = (1.0 / cadence(speed).as_secs_f32()).round() as usize;
            let phase = (0..ticks).fold(0.0, |phase, _| advance(phase, speed));
            (0..4)
                .map(|band| pose(band, phase, speed).dx.abs())
                .sum::<f32>()
        };
        assert!(travel(100) > 2.5 * travel(50));
        assert!(travel(50) > travel(10));
        assert!(cadence(100) >= Duration::from_millis(40));
        assert!(cadence(1) <= Duration::from_millis(100));
    }

    #[test]
    fn band_ends_stay_outside_the_window_at_full_motion() {
        let windows = [
            (720.0, 520.0),
            (1400.0, 950.0),
            (2560.0, 1080.0),
            (3840.0, 1600.0),
            (1080.0, 1920.0),
            (1440.0, 2560.0),
        ];
        for (width, height) in windows {
            for (band, &(cx, _, w, h, rest)) in PLACEMENT.iter().enumerate() {
                for step in 0..=2880 {
                    let p = pose(band, step as f32 / 2880.0 * TAU * 25.0, 100);
                    let half_w = (w * width + 0.4 * height) * (1.0 + p.grow_x) / 2.0;
                    let half_h = h * height * (1.0 + p.grow_y) / 2.0;
                    let (sin, cos) = (rest + p.turn).to_radians().sin_cos();
                    let centre = width * (cx + p.dx);
                    for side in [-1.0f32, 1.0] {
                        for edge in [-1.0f32, 1.0] {
                            let x = centre + side * half_w * cos - edge * half_h * sin;
                            assert!(
                                if side < 0.0 { x < 0.0 } else { x > width },
                                "band {band} end enters a {width}x{height} window at step {step}"
                            );
                        }
                    }
                }
            }
        }
    }
}
