//! Advance a deterministic clock and inspect actual software-rendered pixels.
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, LogicalSize, Rgb8Pixel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
slint::slint! {
    import { Spinner, ProgressBar } from "ui/components.slint";
    import { Theme } from "ui/theme.slint";
    export { Theme }
    export component LoadingFixture inherits Window {
        width: 280px; height: 130px; background: #101116;
        in property <bool> busy: true;
        in property <float> fraction: 0.4;
        if root.busy: Spinner { x: 16px; y: 16px; size: 24px; }
        if root.busy: ProgressBar { x: 16px; y: 64px; width: 240px; progress: -1; label: "Checking files"; }
        ProgressBar { x: 16px; y: 96px; width: 240px; progress: root.fraction; label: "4 of 10"; }
    }
}
thread_local! {
    static NOW: Cell<Duration> = const { Cell::new(Duration::ZERO) };
    static ADAPTER: RefCell<Option<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(None) };
}
struct TestPlatform;
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        ADAPTER.with(|slot| *slot.borrow_mut() = Some(adapter.clone()));
        Ok(adapter)
    }
    fn duration_since_start(&self) -> Duration {
        NOW.with(Cell::get)
    }
}
fn frame(app: &LoadingFixture, at: u64, name: &str) -> Vec<Rgb8Pixel> {
    NOW.with(|now| now.set(Duration::from_millis(at)));
    slint::platform::update_timers_and_animations();
    let width = app.window().size().width as usize;
    let height = app.window().size().height as usize;
    let mut pixels = vec![Rgb8Pixel::default(); width * height];
    ADAPTER.with(|slot| {
        let adapter = slot.borrow();
        let adapter = adapter.as_ref().unwrap();
        adapter.request_redraw();
        adapter.draw_if_needed(|r| {
            r.render(&mut pixels, width);
        });
    });
    if let Some(dir) = std::env::var_os("AURORA_LOADING_OUT") {
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        for p in &pixels {
            bytes.extend_from_slice(&[p.r, p.g, p.b]);
        }
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.ppm")),
            bytes,
        )
        .unwrap();
    }
    pixels
}
fn region(pixels: &[Rgb8Pixel], x: usize, y: usize, w: usize, h: usize) -> Vec<[u8; 3]> {
    (y..y + h)
        .flat_map(|row| {
            (x..x + w).map(move |col| {
                let p = pixels[row * 280 + col];
                [p.r, p.g, p.b]
            })
        })
        .collect()
}
#[test]
fn shared_busy_indicators_animate_in_software_and_stop_under_reduced_motion() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
    let app = LoadingFixture::new().unwrap();
    app.window().set_size(LogicalSize::new(280., 130.));
    app.show().unwrap();
    let a = frame(&app, 1000, "loading-a");
    frame(&app, 1100, "loading-transition");
    let b = frame(&app, 1190, "loading-b");
    assert_ne!(
        region(&a, 16, 16, 24, 24),
        region(&b, 16, 16, 24, 24),
        "busy dots must visibly animate"
    );
    assert_ne!(
        region(&a, 16, 64, 240, 4),
        region(&b, 16, 64, 240, 4),
        "unknown progress must move"
    );
    assert_eq!(
        region(&a, 16, 96, 240, 4),
        region(&b, 16, 96, 240, 4),
        "known progress must remain at its actual value"
    );
    assert!(
        region(&b, 16, 64, 240, 4).iter().any(|p| p[0] > 100),
        "unknown progress cannot be an empty track"
    );
    app.set_fraction(0.75);
    frame(&app, 1400, "loading-progress-transition");
    let filled = frame(&app, 1600, "loading-progress-updated");
    assert_ne!(
        region(&b, 160, 96, 30, 4),
        region(&filled, 160, 96, 30, 4),
        "new measured progress must update the fill"
    );
    app.global::<Theme>().set_reduce_motion(true);
    let c = frame(&app, 2000, "loading-reduced-a");
    let d = frame(&app, 2800, "loading-reduced-b");
    assert_eq!(c, d, "reduced motion must be stable");
    assert!(
        !app.window().has_active_animations(),
        "reduced motion must not keep the frame clock running"
    );
    app.set_busy(false);
    app.global::<Theme>().set_reduce_motion(false);
    frame(&app, 3200, "loading-idle");
    frame(&app, 3500, "loading-idle-settled");
    assert!(
        !app.window().has_active_animations(),
        "idle UI must stop animation work"
    );
}
