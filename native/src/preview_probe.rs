//! Explicit acceptance-build-only input→image-handoff probe. The legacy mode
//! reproduces the old 16 ms/UI-thread scheduling with the SAME new rasterizer,
//! isolating scheduling latency. Neither mode measures physical presentation.
use crate::{AppWindow, Nav, Page, Shell, Skins};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

struct Probe {
    legacy: bool,
    input: Instant,
    previous: Instant,
    inputs: usize,
    samples: Vec<[f64; 3]>,
    intervals: Vec<f64>,
    timer: slint::Timer,
}
thread_local! { static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) }; }
pub fn configure_window(window: &AppWindow) {
    if let Some(arg) = std::env::args().find(|a| a.starts_with("--diagnostic-size=")) {
        let (width, height) = arg
            .split_once('=')
            .unwrap()
            .1
            .split_once('x')
            .expect("WIDTHxHEIGHT");
        let (width, height): (u32, u32) = (width.parse().unwrap(), height.parse().unwrap());
        assert!((720..=2560).contains(&width) && (520..=1600).contains(&height));
        window
            .window()
            .set_size(slint::LogicalSize::new(width as f32, height as f32));
    }
}
pub fn legacy() -> bool {
    PROBE.with(|p| p.borrow().as_ref().is_some_and(|p| p.legacy))
}
pub fn input_time() -> Instant {
    PROBE.with(|p| p.borrow().as_ref().map_or_else(Instant::now, |p| p.input))
}
pub fn frame(input: Instant, render: Duration, handoff: Duration) {
    PROBE.with(|p| {
        if let Some(p) = p.borrow_mut().as_mut() {
            if p.inputs > 30 && p.samples.len() < 600 {
                p.samples.push([
                    input.elapsed().as_secs_f64() * 1000.,
                    render.as_secs_f64() * 1000.,
                    handoff.as_secs_f64() * 1000.,
                ]);
            }
        }
    });
}
pub fn start(window: &AppWindow) -> bool {
    let Some(arg) = std::env::args().find(|a| a.starts_with("--preview-probe=")) else {
        return false;
    };
    let mode = arg.split_once('=').unwrap().1;
    assert!(matches!(mode, "legacy" | "worker"));
    window.set_window_title(format!("Aurora preview probe ({mode}, synthetic)").into());
    window.global::<Shell>().set_loaded(true);
    window.global::<Nav>().set_page(Page::Skins);
    crate::skins::probe_scene();
    PROBE.with(|p| {
        *p.borrow_mut() = Some(Probe {
            legacy: mode == "legacy",
            input: Instant::now(),
            previous: Instant::now(),
            inputs: 0,
            samples: Vec::with_capacity(600),
            intervals: Vec::with_capacity(240),
            timer: slint::Timer::default(),
        })
    });
    PROBE.with(|p| {
        p.borrow().as_ref().unwrap().timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            || {
                let done = PROBE.with(|p| {
                    let mut slot = p.borrow_mut();
                    let p = slot.as_mut().unwrap();
                    let now = Instant::now();
                    if p.inputs > 30 {
                        p.intervals
                            .push(now.duration_since(p.previous).as_secs_f64() * 1000.);
                    }
                    p.previous = now;
                    p.input = now;
                    p.inputs += 1;
                    p.inputs > 240
                });
                if done {
                    PROBE.with(|p| p.borrow().as_ref().unwrap().timer.stop());
                    slint::Timer::single_shot(Duration::from_millis(250), finish);
                } else {
                    if let Some(window) = crate::ui::with(|w, _| w.clone_strong()) {
                        window.global::<Skins>().invoke_rotate(3., 0.);
                    }
                }
            },
        )
    });
    true
}
fn stats(mut values: Vec<f64>) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    let at = |q: f64| {
        values
            .get(((values.len().saturating_sub(1)) as f64 * q).round() as usize)
            .copied()
            .unwrap_or(0.)
    };
    serde_json::json!({"p50":at(0.5),"p95":at(0.95),"max":at(1.)})
}
fn finish() {
    PROBE.with(|p| {
        let slot = p.borrow();
        let p = slot.as_ref().unwrap();
        println!(
            "{}",
            serde_json::json!({
                "mode":if p.legacy {"legacy-scheduler-same-rasterizer"} else {"worker"},
                "input_callbacks":240, "warmup_callbacks":30, "frames_after_warmup":p.samples.len(),
                "input_to_image_handoff_ms":stats(p.samples.iter().map(|v|v[0]).collect()),
                "cpu_render_ms":stats(p.samples.iter().map(|v|v[1]).collect()),
                "ui_image_copy_ms":stats(p.samples.iter().map(|v|v[2]).collect()),
                "input_timer_interval_ms":stats(p.intervals.clone()),
                "physical_presentation_measured":false
            })
        );
    });
    let _ = slint::quit_event_loop();
}
