//! Borealis window background for the GPUI host: the same cached,
//! pre-blurred aurora band layers as the Slint host (`native/ui/borealis.slint`,
//! `native/src/borealis.rs`), with the same rest placements, pose math,
//! cadence and animation gate.
//!
//! GPUI has no rotated-image primitive, so the static base and the four bands
//! are composed on the Tokio runtime into one small opaque image (pose math
//! evaluated per pixel, bilinear, premultiplied "over" in the Slint draw order)
//! that the view paints stretched over its parent. The image is far smaller
//! than the window; the bands are already heavily blurred, so the GPU's linear
//! upscale stays soft. No work happens in `render`.
//!
//! Motion contract (hard requirement): the only recurring work is one
//! foreground [`Task`] that exists exactly while [`should_animate`] holds
//! (speed above 0, background enabled, window visible and not minimized,
//! accelerated host). Dropping the task stops it completely: no timers and no
//! redraw requests remain at speed 0, with the background off, while hidden
//! or minimized, or on a software renderer, which keeps the static rest pose.
//! Phase steps are fixed per tick, so a delayed or hidden window never
//! catches up; at most one composition is in flight and newer poses coalesce.

#![allow(dead_code)]

use std::f32::consts::TAU;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{
    Context, IntoElement, ObjectFit, ParentElement as _, Render, RenderImage, Styled as _,
    StyledImage as _, Subscription, Task, Window, div, img,
};
use tokio::runtime::Handle;

use crate::api::Appearance;
use crate::images;

// ---------------------------------------------------------------------------
// Pose math (ported verbatim from native/src/borealis.rs)
// ---------------------------------------------------------------------------

/// Displacement of one band from its rest placement: offsets are fractions of
/// the window, turn is degrees, grow is added scale. All zero is the rest pose.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub dx: f32,
    pub dy: f32,
    pub turn: f32,
    pub grow_x: f32,
    pub grow_y: f32,
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

/// Rest placement mirrored from the BorealisBackdrop in borealis.slint, per
/// band: centre x/y as window fractions, width as window-width fraction (plus
/// 0.4 of the window height), height as window-height fraction, rest turn in
/// degrees.
const PLACEMENT: [(f32, f32, f32, f32, f32); 4] = [
    (0.42, 0.66, 1.70, 0.78, -14.0),
    (0.72, 0.14, 1.85, 0.70, 10.0),
    (0.58, 0.90, 1.65, 0.70, -6.0),
    (0.48, 0.42, 1.60, 0.52, 10.0),
];

/// Slint draws band 2, then 1, then 0, then 3 over the base.
const DRAW_ORDER: [usize; 4] = [2, 1, 0, 3];

/// Timer period: 10 Hz at the lowest nonzero speed, 24 Hz at 100. The bands
/// are heavily blurred, so the coarser steps of slow speeds stay soft.
pub fn cadence(speed: u8) -> Duration {
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

pub fn advance(phase: f32, speed: u8) -> f32 {
    (phase + velocity(speed) * cadence(speed).as_secs_f32()) % PERIOD
}

/// Each wave is measured from phase 0, so phase 0 (and motion 0) is exactly
/// the all-zero rest pose and resets never jump.
pub fn pose(band: usize, phase: f32, speed: u8) -> Pose {
    if speed == 0 {
        return Pose::default();
    }
    let reach = reach(speed);
    let wave = |(multiplier, offset, extent): Wave| {
        reach * extent * ((phase * multiplier + offset).sin() - offset.sin())
    };
    let [dx, dy, turn, grow_x, grow_y] = WAVES[band].map(wave);
    Pose {
        dx,
        dy,
        turn,
        grow_x,
        grow_y,
    }
}

/// Alphas of the theme surfaces over Borealis, from `native/src/theme.rs`:
/// the sidebar, content panels, hover and sunken surfaces show the bands
/// softly, while raised layers (dialogs, menus) stay nearly opaque so text
/// never sits on a bright area. The theme owner applies these when the saved
/// background is [`is_borealis`].
pub const SIDEBAR_ALPHA: u8 = 0x8c;
pub const SURFACE_ALPHA: u8 = 0xb4;
pub const HOVER_ALPHA: u8 = 0xd0;
pub const SUNKEN_ALPHA: u8 = 0xc8;
pub const RAISED_ALPHA: u8 = 0xf2;

/// The saved background ids, in the order of the Settings dropdown.
pub const BACKGROUNDS: [&str; 2] = ["simple", "borealis"];

/// Whether the persisted background is the stylized Borealis layer.
pub fn is_borealis(background: &str) -> bool {
    background == "borealis"
}

/// The animation gate. Everything else (timers, phase, redraws) follows from
/// this alone, so speed 0 or any false input means nothing is scheduled.
pub fn should_animate(speed: u8, enabled: bool, visible: bool, accelerated: bool) -> bool {
    speed > 0 && enabled && visible && accelerated
}

/// Whether the host renders on a real GPU. A software adapter keeps Borealis
/// static, like the Slint host's non-OpenGL renderers.
pub fn host_accelerated(window: &Window) -> bool {
    window
        .gpu_specs()
        .is_some_and(|specs| !specs.is_software_emulated)
}

// ---------------------------------------------------------------------------
// Motion state machine (port of native/src/borealis.rs `sync` / `tick`)
// ---------------------------------------------------------------------------

/// What the owner must do with its single phase source after a resync.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Schedule {
    /// Nothing may be scheduled: drop the task.
    Stop,
    /// The running source already has the right cadence.
    Keep,
    /// (Re)start the source with this period, dropping any previous one.
    Start(Duration),
}

/// Result of one phase tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tick {
    /// The phase advanced one fixed step; publish the new pose.
    Advanced,
    /// The inputs changed under the source; resync instead of advancing.
    Resync,
}

#[derive(Clone, Debug, Default)]
pub struct Motion {
    speed: u8,
    enabled: bool,
    visible: bool,
    accelerated: bool,
    phase: f32,
    interval: Duration,
    running: bool,
}

impl Motion {
    pub fn speed(&self) -> u8 {
        self.speed
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn phase(&self) -> f32 {
        self.phase
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn run(&self) -> bool {
        should_animate(self.speed, self.enabled, self.visible, self.accelerated)
    }

    /// Resynchronizes after appearance, renderer and visibility events.
    /// Hidden accelerated windows retain their phase but never catch up on
    /// resume.
    pub fn sync(&mut self) -> Schedule {
        let run = self.run();
        if self.speed == 0 || !self.enabled || !self.accelerated {
            self.phase = 0.0;
        }
        if !run {
            self.running = false;
            return Schedule::Stop;
        }
        let interval = cadence(self.speed);
        if self.running && self.interval == interval {
            return Schedule::Keep;
        }
        self.running = true;
        self.interval = interval;
        Schedule::Start(interval)
    }

    /// One tick of the phase source. Fixed phase steps: delayed frames and
    /// hidden windows never catch up.
    pub fn tick(&mut self) -> Tick {
        if !self.run() || self.interval != cadence(self.speed) {
            return Tick::Resync;
        }
        self.phase = advance(self.phase, self.speed);
        Tick::Advanced
    }

    pub fn poses(&self) -> [Pose; 4] {
        [0, 1, 2, 3].map(|band| pose(band, self.phase, self.speed))
    }

    pub fn set_speed(&mut self, speed: u8) -> bool {
        let speed = speed.min(100);
        std::mem::replace(&mut self.speed, speed) != speed
    }

    pub fn set_enabled(&mut self, enabled: bool) -> bool {
        std::mem::replace(&mut self.enabled, enabled) != enabled
    }

    pub fn set_visible(&mut self, visible: bool) -> bool {
        std::mem::replace(&mut self.visible, visible) != visible
    }

    pub fn set_accelerated(&mut self, accelerated: bool) -> bool {
        std::mem::replace(&mut self.accelerated, accelerated) != accelerated
    }
}

// ---------------------------------------------------------------------------
// Band layers and composition
// ---------------------------------------------------------------------------

/// A decoded layer, premultiplied RGBA8, sampled bilinearly like a smooth
/// Slint `Image` (clamped at the edges).
struct Raster {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>,
}

impl Raster {
    fn decode(png: &[u8]) -> Option<Self> {
        let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .ok()?
            .to_rgba8();
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            return None;
        }
        let premultiply =
            |channel: u8, alpha: u8| ((u16::from(channel) * u16::from(alpha) + 127) / 255) as u8;
        let pixels = image
            .pixels()
            .map(|pixel| {
                let [r, g, b, a] = pixel.0;
                [premultiply(r, a), premultiply(g, a), premultiply(b, a), a]
            })
            .collect();
        Some(Self {
            width: width as usize,
            height: height as usize,
            pixels,
        })
    }

    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = u * self.width as f32 - 0.5;
        let y = v * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |x: f32, y: f32| {
            let x = (x as isize).clamp(0, self.width as isize - 1) as usize;
            let y = (y as isize).clamp(0, self.height as isize - 1) as usize;
            self.pixels[y * self.width + x]
        };
        let corners = [
            (at(x0, y0), (1.0 - fx) * (1.0 - fy)),
            (at(x0 + 1.0, y0), fx * (1.0 - fy)),
            (at(x0, y0 + 1.0), (1.0 - fx) * fy),
            (at(x0 + 1.0, y0 + 1.0), fx * fy),
        ];
        let mut out = [0.0; 4];
        for (pixel, weight) in corners {
            for (out, channel) in out.iter_mut().zip(pixel) {
                *out += f32::from(channel) * weight / 255.0;
            }
        }
        out
    }
}

/// The base and the four bands, decoded once from the Slint host's assets.
struct Layers {
    base: Raster,
    bands: [Raster; 4],
}

impl Layers {
    fn decode() -> Option<Self> {
        Some(Self {
            base: Raster::decode(include_bytes!("../../native/assets/borealis-base.png"))?,
            bands: [
                Raster::decode(include_bytes!("../../native/assets/borealis-band-0.png"))?,
                Raster::decode(include_bytes!("../../native/assets/borealis-band-1.png"))?,
                Raster::decode(include_bytes!("../../native/assets/borealis-band-2.png"))?,
                Raster::decode(include_bytes!("../../native/assets/borealis-band-3.png"))?,
            ],
        })
    }
}

/// Longest side of the composed image. The source bands are at most 900 px
/// wide and pre-blurred, so more resolution would add cost, not detail.
const MAX_SIDE: f32 = 448.0;

/// Composition size for a window: the window's aspect at most [`MAX_SIDE`].
fn output_size(window: (f32, f32)) -> (usize, usize) {
    let scale = (MAX_SIDE / window.0.max(window.1)).min(1.0);
    (
        ((window.0 * scale).round() as usize).max(1),
        ((window.1 * scale).round() as usize).max(1),
    )
}

/// Composes the opaque RGBA8 frame for `poses` in a window of `window`
/// logical pixels, laid out like the BorealisBackdrop: a base filled to the
/// window, then each band scaled (by its stretch), turned about its own
/// centre, and placed at its rest placement plus drift.
fn compose(
    layers: &Layers,
    window: (f32, f32),
    (out_width, out_height): (usize, usize),
    poses: &[Pose; 4],
) -> Vec<u8> {
    struct Placed<'a> {
        cx: f32,
        cy: f32,
        cos: f32,
        sin: f32,
        inv_width: f32,
        inv_height: f32,
        raster: &'a Raster,
    }
    let (win_width, win_height) = window;
    let placed = DRAW_ORDER.map(|band| {
        let (x, y, width, height, rest) = PLACEMENT[band];
        let pose = poses[band];
        let band_width = (width * win_width + 0.4 * win_height) * (1.0 + pose.grow_x);
        let band_height = height * win_height * (1.0 + pose.grow_y);
        let (sin, cos) = (rest + pose.turn).to_radians().sin_cos();
        Placed {
            cx: win_width * (x + pose.dx),
            cy: win_height * (y + pose.dy),
            cos,
            sin,
            inv_width: 1.0 / band_width,
            inv_height: 1.0 / band_height,
            raster: &layers.bands[band],
        }
    });
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let mut rgba = Vec::with_capacity(out_width * out_height * 4);
    for row in 0..out_height {
        let v = (row as f32 + 0.5) / out_height as f32;
        let y = v * win_height;
        for column in 0..out_width {
            let u = (column as f32 + 0.5) / out_width as f32;
            let x = u * win_width;
            let mut colour = layers.base.sample(u, v);
            for band in &placed {
                let (dx, dy) = (x - band.cx, y - band.cy);
                let local_x = (dx * band.cos + dy * band.sin) * band.inv_width + 0.5;
                let local_y = (dy * band.cos - dx * band.sin) * band.inv_height + 0.5;
                if !(0.0..=1.0).contains(&local_x) || !(0.0..=1.0).contains(&local_y) {
                    continue;
                }
                let sample = band.raster.sample(local_x, local_y);
                let keep = 1.0 - sample[3];
                for (channel, source) in colour.iter_mut().zip(sample) {
                    *channel = source + *channel * keep;
                }
            }
            rgba.extend([byte(colour[0]), byte(colour[1]), byte(colour[2]), 255]);
        }
    }
    rgba
}

/// Composes and converts one frame for GPUI.
fn render_frame(
    layers: &Layers,
    window: (f32, f32),
    poses: &[Pose; 4],
) -> Option<Arc<RenderImage>> {
    let size = output_size(window);
    let rgba = compose(layers, window, size, poses);
    images::rgba_image(size.0 as u32, size.1 as u32, rgba)
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

enum LayerState {
    /// Not needed (background off) or not requested yet.
    Idle,
    Loading,
    Ready(Arc<Layers>),
    /// The embedded assets failed to decode; the background stays plain.
    Failed,
}

/// The Borealis background view. Create one entity for the window, keep it
/// behind the root content (it fills its parent absolutely) and feed it the
/// appearance and window events documented on the setters.
pub struct Borealis {
    rt: Handle,
    motion: Motion,
    /// Window size in logical pixels; the composition geometry.
    viewport: (f32, f32),
    layers: LayerState,
    image: Option<Arc<RenderImage>>,
    /// Replaced frames; their atlas entries are released on the next render.
    retired: Vec<Arc<RenderImage>>,
    /// Bumped whenever results in flight stop being wanted.
    epoch: u64,
    composing: bool,
    /// A newer pose or size arrived while a composition was in flight.
    dirty: bool,
    /// The size and poses of the latest dispatched composition.
    requested: Option<((f32, f32), [Pose; 4])>,
    /// The single phase source; dropping it stops all animation work.
    ticker: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Borealis {
    /// Builds the (initially disabled, static) view. Visibility, window size
    /// and renderer acceleration are read from `window` and then followed
    /// through its own observers; nothing is scheduled until the background
    /// is enabled.
    pub fn new(runtime: Handle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut motion = Motion::default();
        motion.set_visible(window.is_visible());
        motion.set_accelerated(host_accelerated(window));
        let subscriptions = vec![
            cx.observe_window_visibility(window, |this, visibility, window, cx| {
                this.set_accelerated(host_accelerated(window), cx);
                this.set_window_active(visibility.is_visible(), cx);
            }),
            cx.observe_window_bounds(window, |this, window, cx| {
                this.set_viewport(window, cx);
            }),
        ];
        let size = window.viewport_size();
        Self {
            rt: runtime,
            motion,
            viewport: (f32::from(size.width), f32::from(size.height)),
            layers: LayerState::Idle,
            image: None,
            retired: Vec::new(),
            epoch: 0,
            composing: false,
            dirty: false,
            requested: None,
            ticker: None,
            _subscriptions: subscriptions,
        }
    }

    /// Whether the phase source is currently scheduled (diagnostics only).
    pub fn animating(&self) -> bool {
        self.ticker.is_some()
    }

    /// Applies a complete appearance snapshot: the saved background and the
    /// saved motion speed. Use at startup; afterwards prefer the narrower
    /// setters so an unreleased Settings slider preview is not overwritten.
    pub fn set_appearance(&mut self, appearance: &Appearance, cx: &mut Context<Self>) {
        let changed = self.motion.set_enabled(is_borealis(&appearance.background));
        let changed = self.motion.set_speed(appearance.aurora_motion_speed) | changed;
        if changed {
            self.sync(cx);
        }
    }

    /// The saved background is (or is no longer) Borealis.
    pub fn set_background(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.motion.set_enabled(enabled) {
            self.sync(cx);
        }
    }

    /// The effective Motion speed (0 to 100): a live slider preview, the saved
    /// value, or a restored one.
    pub fn set_motion(&mut self, speed: u8, cx: &mut Context<Self>) {
        if self.motion.set_speed(speed) {
            self.sync(cx);
        }
    }

    /// The window is (or is no longer) presented: shown and not minimized.
    /// Call from the root's visibility/minimize events if the view's own
    /// observer is not enough. A hidden window keeps its phase.
    pub fn set_window_active(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.motion.set_visible(visible) {
            self.sync(cx);
        }
    }

    /// The host renders (or no longer renders) on a real GPU.
    pub fn set_accelerated(&mut self, accelerated: bool, cx: &mut Context<Self>) {
        if self.motion.set_accelerated(accelerated) {
            self.sync(cx);
        }
    }

    fn set_viewport(&mut self, window: &Window, cx: &mut Context<Self>) {
        let size = window.viewport_size();
        let viewport = (f32::from(size.width), f32::from(size.height));
        // A minimized window reports an empty client area; keep the last one.
        if viewport.0 < 1.0 || viewport.1 < 1.0 || viewport == self.viewport {
            return;
        }
        self.viewport = viewport;
        if self.motion.enabled() {
            self.request_render(cx);
        }
    }

    /// Applies the gate: starts, restarts or drops the single phase source,
    /// then republishes the pose (also after speed changes, without advancing
    /// a paused phase).
    fn sync(&mut self, cx: &mut Context<Self>) {
        match self.motion.sync() {
            Schedule::Stop => self.ticker = None,
            Schedule::Keep => {}
            Schedule::Start(interval) => self.start_ticker(interval, cx),
        }
        self.request_render(cx);
    }

    fn start_ticker(&mut self, interval: Duration, cx: &mut Context<Self>) {
        // Replacing the task drops (cancels) the previous one.
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        match self.motion.tick() {
            Tick::Advanced => self.request_render(cx),
            Tick::Resync => self.sync(cx),
        }
    }

    /// Releases the frame and everything in flight when the background is off.
    fn release(&mut self, cx: &mut Context<Self>) {
        self.epoch += 1;
        self.dirty = false;
        self.requested = None;
        self.layers = LayerState::Idle;
        if let Some(image) = self.image.take() {
            self.retired.push(image);
            cx.notify();
        }
    }

    /// Wants a frame for the current pose and size. One composition runs at a
    /// time; anything newer coalesces into a single follow-up.
    fn request_render(&mut self, cx: &mut Context<Self>) {
        if !self.motion.enabled() {
            self.release(cx);
            return;
        }
        let layers = match &self.layers {
            LayerState::Ready(layers) => layers.clone(),
            LayerState::Idle => {
                self.load_layers(cx);
                return;
            }
            LayerState::Loading | LayerState::Failed => return,
        };
        let window = self.viewport;
        let poses = self.motion.poses();
        // Resyncs that change neither the pose nor the size cost nothing.
        if self.requested == Some((window, poses)) {
            return;
        }
        if self.composing {
            self.dirty = true;
            return;
        }
        self.composing = true;
        self.dirty = false;
        self.requested = Some((window, poses));
        let epoch = self.epoch;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn_blocking(move || {
            let _ = tx.send(render_frame(&layers, window, &poses));
        });
        cx.spawn(async move |this, cx| {
            if let Ok(frame) = rx.await {
                let _ = this.update(cx, |this, cx| this.finish_render(epoch, frame, cx));
            }
        })
        .detach();
    }

    fn finish_render(
        &mut self,
        epoch: u64,
        frame: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        self.composing = false;
        if epoch == self.epoch && self.motion.enabled() {
            if let Some(frame) = frame {
                if let Some(old) = self.image.replace(frame) {
                    self.retired.push(old);
                }
                cx.notify();
            }
        }
        if self.dirty || epoch != self.epoch {
            self.request_render(cx);
        }
    }

    fn load_layers(&mut self, cx: &mut Context<Self>) {
        self.layers = LayerState::Loading;
        let epoch = self.epoch;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn_blocking(move || {
            let _ = tx.send(Layers::decode().map(Arc::new));
        });
        cx.spawn(async move |this, cx| {
            if let Ok(layers) = rx.await {
                let _ = this.update(cx, |this, cx| {
                    // A newer epoch means the background was switched off (and
                    // maybe on again) meanwhile; that request reloads itself.
                    if epoch != this.epoch || !matches!(this.layers, LayerState::Loading) {
                        return;
                    }
                    this.layers = match layers {
                        Some(layers) => LayerState::Ready(layers),
                        None => LayerState::Failed,
                    };
                    this.request_render(cx);
                });
            }
        })
        .detach();
    }
}

impl Render for Borealis {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.retired.drain(..) {
            window.drop_image(old).ok();
        }
        let layer = div().absolute().top_0().left_0().size_full();
        match (&self.image, self.motion.enabled()) {
            (Some(image), true) => {
                layer.child(img(image.clone()).size_full().object_fit(ObjectFit::Fill))
            }
            _ => layer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_zero_and_phase_zero_are_the_rest_pose() {
        for band in 0..4 {
            assert_eq!(pose(band, 1.3, 0), Pose::default());
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

    #[test]
    fn animation_gate_truth_table() {
        for speed in [0u8, 1, 50, 100] {
            for enabled in [false, true] {
                for visible in [false, true] {
                    for accelerated in [false, true] {
                        let expected = speed > 0 && enabled && visible && accelerated;
                        assert_eq!(
                            should_animate(speed, enabled, visible, accelerated),
                            expected,
                            "speed {speed} enabled {enabled} visible {visible} accelerated {accelerated}"
                        );
                        let mut motion = Motion::default();
                        motion.set_speed(speed);
                        motion.set_enabled(enabled);
                        motion.set_visible(visible);
                        motion.set_accelerated(accelerated);
                        let schedule = motion.sync();
                        assert_eq!(motion.running(), expected);
                        assert_eq!(
                            schedule,
                            if expected {
                                Schedule::Start(cadence(speed))
                            } else {
                                Schedule::Stop
                            }
                        );
                    }
                }
            }
        }
    }

    fn running(speed: u8) -> Motion {
        let mut motion = Motion::default();
        motion.set_speed(speed);
        motion.set_enabled(true);
        motion.set_visible(true);
        motion.set_accelerated(true);
        assert_eq!(motion.sync(), Schedule::Start(cadence(speed)));
        motion
    }

    #[test]
    fn nothing_is_scheduled_at_speed_zero_and_the_phase_resets() {
        let mut motion = running(60);
        assert_eq!(motion.tick(), Tick::Advanced);
        assert!(motion.phase() > 0.0);
        motion.set_speed(0);
        assert_eq!(motion.sync(), Schedule::Stop);
        assert!(!motion.running());
        assert_eq!(motion.phase(), 0.0);
        assert_eq!(motion.poses(), [Pose::default(); 4]);
        // A stray tick from a source that has not been dropped yet only asks
        // for a resync; it never advances.
        assert_eq!(motion.tick(), Tick::Resync);
        assert_eq!(motion.phase(), 0.0);
        // Repeated syncs stay stopped.
        assert_eq!(motion.sync(), Schedule::Stop);
    }

    #[test]
    fn hidden_windows_keep_their_phase_and_never_catch_up() {
        let mut motion = running(100);
        for _ in 0..5 {
            assert_eq!(motion.tick(), Tick::Advanced);
        }
        let held = motion.phase();
        motion.set_visible(false);
        assert_eq!(motion.sync(), Schedule::Stop);
        assert_eq!(motion.phase(), held, "hiding keeps the phase");
        assert_eq!(motion.tick(), Tick::Resync);
        assert_eq!(motion.phase(), held);

        motion.set_visible(true);
        assert_eq!(motion.sync(), Schedule::Start(cadence(100)));
        assert_eq!(motion.phase(), held, "resuming does not jump");
        assert_eq!(motion.tick(), Tick::Advanced);
        assert_eq!(motion.phase(), advance(held, 100), "one fixed step only");
    }

    #[test]
    fn software_hosts_and_disabled_backgrounds_stay_at_the_rest_pose() {
        for (enabled, accelerated) in [(true, false), (false, true), (false, false)] {
            let mut motion = running(80);
            motion.tick();
            motion.set_enabled(enabled);
            motion.set_accelerated(accelerated);
            assert_eq!(motion.sync(), Schedule::Stop);
            assert_eq!(motion.phase(), 0.0);
            assert_eq!(motion.poses().map(|p| p.turn), [0.0; 4]);
        }
    }

    #[test]
    fn a_speed_change_restarts_the_source_only_when_the_cadence_differs() {
        let mut motion = running(100);
        assert_eq!(motion.sync(), Schedule::Keep);
        motion.set_speed(99);
        assert_eq!(
            cadence(100),
            cadence(99),
            "adjacent speeds can share a cadence"
        );
        assert_eq!(motion.sync(), Schedule::Keep);
        motion.set_speed(10);
        assert_ne!(cadence(99), cadence(10));
        assert_eq!(motion.tick(), Tick::Resync, "the old source defers to sync");
        assert_eq!(motion.sync(), Schedule::Start(cadence(10)));
        assert_eq!(motion.tick(), Tick::Advanced);
        assert_eq!(motion.speed(), 10);
        assert!(motion.set_speed(200), "out-of-range speeds clamp");
        assert_eq!(motion.speed(), 100);
    }

    fn solid(width: usize, height: usize, rgba: [u8; 4]) -> Raster {
        let [r, g, b, a] = rgba;
        let premultiply = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
        Raster {
            width,
            height,
            pixels: vec![[premultiply(r), premultiply(g), premultiply(b), a]; width * height],
        }
    }

    fn clear() -> Raster {
        solid(1, 1, [0, 0, 0, 0])
    }

    fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let at = (y * width + x) * 4;
        [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
    }

    #[test]
    fn composition_places_a_rotated_band_over_the_base_in_draw_order() {
        let layers = Layers {
            base: solid(1, 1, [0, 0, 0, 255]),
            // Only band 0 is visible; it is opaque white.
            bands: [solid(1, 1, [255, 255, 255, 255]), clear(), clear(), clear()],
        };
        let window = (800.0, 600.0);
        let rgba = compose(&layers, window, (80, 60), &[Pose::default(); 4]);
        assert_eq!(rgba.len(), 80 * 60 * 4);
        // Band 0 is centred at (0.42, 0.66) and turned -14 degrees; its centre
        // is covered, the top-left corner lies past its upper edge.
        assert_eq!(pixel(&rgba, 80, 34, 40), [255, 255, 255, 255]);
        assert_eq!(pixel(&rgba, 80, 0, 0), [0, 0, 0, 255]);

        // A pose displaces the band: pushed far right it uncovers the centre.
        let mut poses = [Pose::default(); 4];
        poses[0].dx = 1.5;
        let moved = compose(&layers, window, (80, 60), &poses);
        assert_eq!(pixel(&moved, 80, 34, 40), [0, 0, 0, 255]);
    }

    #[test]
    fn composition_blends_premultiplied_bands_over_an_opaque_base() {
        let layers = Layers {
            base: solid(1, 1, [0, 100, 200, 255]),
            bands: [clear(), clear(), solid(1, 1, [255, 0, 0, 128]), clear()],
        };
        // Band 2 covers the window centre; half-transparent red over the base.
        let rgba = compose(&layers, (800.0, 600.0), (8, 6), &[Pose::default(); 4]);
        let [r, g, b, a] = pixel(&rgba, 8, 4, 4);
        assert_eq!(a, 255);
        assert!((i32::from(r) - 128).abs() <= 1, "red {r}");
        assert!((i32::from(g) - 50).abs() <= 1, "green {g}");
        assert!((i32::from(b) - 100).abs() <= 1, "blue {b}");
    }

    #[test]
    fn output_size_keeps_the_aspect_and_never_upscales() {
        assert_eq!(output_size((100.0, 50.0)), (100, 50));
        assert_eq!(output_size((1792.0, 896.0)), (448, 224));
        assert_eq!(output_size((900.0, 1800.0)), (224, 448));
        assert_eq!(output_size((1.0, 1.0)), (1, 1));
    }

    #[test]
    fn the_embedded_assets_decode_and_compose_to_an_opaque_frame() {
        let layers = Layers::decode().expect("embedded Borealis assets decode");
        assert_eq!(
            (layers.base.width, layers.base.height),
            (192, 120),
            "base layer"
        );
        assert_eq!(
            layers.bands.each_ref().map(|b| (b.width, b.height)),
            [(800, 340), (900, 320), (800, 320), (720, 280)]
        );
        let window = (1100.0, 700.0);
        let size = output_size(window);
        let rest = compose(&layers, window, size, &[Pose::default(); 4]);
        assert_eq!(rest.len(), size.0 * size.1 * 4);
        assert!(rest.chunks_exact(4).all(|p| p[3] == 255));
        assert!(
            rest.chunks_exact(4).any(|p| p[..3] != rest[..3]),
            "the frame is not one flat colour"
        );
        let moved = compose(
            &layers,
            window,
            size,
            &[0, 1, 2, 3].map(|band| pose(band, 3.0, 100)),
        );
        assert_ne!(rest, moved, "a different pose changes the frame");
        assert_eq!(rest, compose(&layers, window, size, &[Pose::default(); 4]));
    }

    #[test]
    fn surface_alphas_and_backgrounds_match_the_slint_host() {
        assert_eq!(BACKGROUNDS, ["simple", "borealis"]);
        assert!(is_borealis("borealis") && !is_borealis("simple") && !is_borealis(""));
        assert_eq!(
            [
                SIDEBAR_ALPHA,
                SURFACE_ALPHA,
                HOVER_ALPHA,
                SUNKEN_ALPHA,
                RAISED_ALPHA
            ],
            [0x8c, 0xb4, 0xd0, 0xc8, 0xf2]
        );
    }
}
