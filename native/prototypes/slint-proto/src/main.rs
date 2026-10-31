#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! NATIVE-0 Slint prototype. `--bench` cycles navigation and dialogs and
//! prints render-completion latency to stdout; otherwise it idles normally.

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

slint::include_modules!();

fn item(name: &str, detail: &str, ready: bool, glyph: &str) -> InstanceItem {
    InstanceItem { name: name.into(), detail: detail.into(), ready, glyph: glyph.into() }
}

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    let instances = Rc::new(VecModel::from(vec![
        item("Aurora 3.0", "Minecraft 1.21.10 · Fabric 0.17.3", true, "A"),
        item("Survival", "Minecraft 1.21.8 · Fabric 0.16.14", true, "S"),
        item("Creative build", "Minecraft 1.21.10 · Vanilla", false, "C"),
        item("Modpack test", "Minecraft 1.20.1 · Forge 47.4", true, "M"),
    ]));
    app.set_instances(ModelRc::from(instances.clone()));

    let all_rows: Rc<Vec<InstanceItem>> = Rc::new(
        (0..300)
            .map(|i| item(&format!("Content item {i:03}"), &format!("version 1.{i}.0 · author {}", i % 17), i % 3 != 0, "▣"))
            .collect(),
    );
    let rows = Rc::new(VecModel::from((*all_rows).clone()));
    app.set_content_rows(ModelRc::from(rows.clone()));
    {
        let all_rows = all_rows.clone();
        let rows = rows.clone();
        app.on_query_changed(move |query: SharedString| {
            let q = query.to_lowercase();
            rows.set_vec(all_rows.iter().filter(|r| r.name.to_lowercase().contains(&q)).cloned().collect::<Vec<_>>());
        });
    }

    let weak = app.as_weak();
    app.on_play(move || {
        let weak = weak.clone();
        if let Some(app) = weak.upgrade() {
            app.set_busy(true);
            app.set_status_text("Downloading libraries".into());
        }
        std::thread::spawn(move || {
            for step in 1..=30 {
                std::thread::sleep(Duration::from_millis(100));
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = weak.upgrade() {
                        app.set_progress(step as f32 / 30.0);
                        app.set_status_text(format!("Downloading libraries {step}/30").into());
                        if step == 30 {
                            app.set_busy(false);
                            app.set_status_text("Ready".into());
                        }
                    }
                });
            }
        });
    });

    if std::env::args().any(|a| a == "--bench") {
        bench(&app);
    }
    let _ = instances.row_count();
    app.run()
}

/// Measures time from a state change to the end of the next rendered frame.
fn bench(app: &AppWindow) {
    let rendered: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let r = rendered.clone();
    let _ = app.window().set_rendering_notifier(move |state, _| {
        if let slint::RenderingState::AfterRendering = state {
            *r.lock().unwrap() = Some(Instant::now());
        }
    });
    let weak = app.as_weak();
    let samples: Rc<std::cell::RefCell<Vec<f64>>> = Rc::default();
    let step = Rc::new(std::cell::Cell::new(0u32));
    let pending: Rc<std::cell::Cell<Option<Instant>>> = Rc::default();
    let timer = Box::leak(Box::new(slint::Timer::default()));
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(40), move || {
        let Some(app) = weak.upgrade() else { return };
        if let Some(start) = pending.get() {
            let done = *rendered.lock().unwrap();
            match done {
                Some(t) if t >= start => {
                    samples.borrow_mut().push((t - start).as_secs_f64() * 1000.0);
                    pending.set(None);
                }
                _ => return,
            }
        }
        let n = step.get();
        if n == u32::MAX {
            return;
        }
        if n >= 400 {
            let mut s = samples.borrow().clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let pct = |p: f64| s[((s.len() as f64 - 1.0) * p) as usize];
            println!("bench samples={} p50={:.2}ms p95={:.2}ms max={:.2}ms", s.len(), pct(0.5), pct(0.95), s[s.len() - 1]);
            step.set(u32::MAX);
            app.set_page(0);
            app.set_dialog_open(false);
            return;
        }
        if n == u32::MAX {
            return;
        }
        step.set(n + 1);
        pending.set(Some(Instant::now()));
        match n % 4 {
            0 => app.set_page(1),
            1 => app.set_page(2),
            2 => app.set_dialog_open(!app.get_dialog_open()),
            _ => app.set_page(0),
        }
        app.window().request_redraw();
    });
}
