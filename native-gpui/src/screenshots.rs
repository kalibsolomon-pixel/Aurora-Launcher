//! Event-driven, paged per-instance screenshot gallery. Files and decoding stay
//! behind the typed API; rendering only clones retained GPU images.
use crate::{
    api,
    artwork::ArtworkCache,
    backend::guarded,
    controls::{AuroraButton, AuroraInput, ButtonSize, ButtonVariant},
    images, theme, ui,
};
use aurora_launcher_lib::host::Host;
use gpui::{Context, Entity, RenderImage, SharedString, Window, div, img, prelude::*, px};
use gpui_kit::base::{Dialog, input::InputState};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

const PAGE_SIZE: usize = 12;
type Image = Arc<RenderImage>;

fn fitted_size(source: (f32, f32), bounds: (f32, f32)) -> (f32, f32) {
    let scale = (bounds.0 / source.0).min(bounds.1 / source.1);
    (source.0 * scale, source.1 * scale)
}
fn fitted_image(image: Image, width: f32, height: f32) -> impl IntoElement {
    let source = image.size(0);
    let (width, height) = fitted_size(
        (source.width.0 as f32, source.height.0 as f32),
        (width, height),
    );
    // Give GPUI definite aspect-preserving bounds. Percentage image height
    // and ObjectFit alone can paint at intrinsic aspect beyond a flex well.
    img(image).w(px(width)).h(px(height)).flex_shrink_0()
}

#[derive(Default)]
struct GalleryState {
    instance: Option<String>,
    generation: u64,
    shots: Vec<api::Screenshot>,
    page: usize,
    selected: Option<usize>,
    delete: Option<api::Screenshot>,
    busy: bool,
}
impl GalleryState {
    fn reset(&mut self, instance: Option<String>) {
        self.instance = instance;
        self.generation += 1;
        self.shots.clear();
        self.page = 0;
        self.selected = None;
        self.delete = None;
        self.busy = false;
    }
    fn current(&self) -> Option<&api::Screenshot> {
        self.shots.get(self.selected?)
    }
    fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|&i| i < self.shots.len());
        self.delete = None;
    }
    fn confirm_delete(&self) -> Option<api::Screenshot> {
        self.delete
            .as_ref()
            .filter(|shot| self.current() == Some(*shot))
            .cloned()
    }
    fn range(&self) -> std::ops::Range<usize> {
        let start = (self.page * PAGE_SIZE).min(self.shots.len());
        start..(start + PAGE_SIZE).min(self.shots.len())
    }
    fn accepts(&self, generation: u64) -> bool {
        self.generation == generation
    }
}

enum Reply {
    Listed(u64, api::ApiResult<Vec<api::Screenshot>>),
    Thumbnails(
        u64,
        usize,
        Vec<(api::Screenshot, api::ApiResult<api::ScreenshotBitmap>)>,
    ),
    Preview(u64, u64, api::ApiResult<api::ScreenshotBitmap>),
    Action(u64, &'static str, api::ApiResult<()>),
}

pub struct Screenshots {
    host: Option<Host>,
    runtime: tokio::runtime::Handle,
    tx: async_channel::Sender<Reply>,
    state: GalleryState,
    loading: bool,
    thumbnails_loading: bool,
    cache: ArtworkCache<String>,
    preview: Option<Image>,
    preview_serial: u64,
    preview_inflight: bool,
    preview_error: String,
    message: String,
    rename: Entity<InputState>,
    // Replaced page/instance work exits between images instead of building an
    // unbounded queue when users navigate quickly.
    work: Arc<AtomicU64>,
}
impl Drop for Screenshots {
    fn drop(&mut self) {
        self.work.fetch_add(1, Ordering::Relaxed);
    }
}

fn key(instance: &str, shot: &api::Screenshot) -> String {
    format!(
        "{instance}/{}:{}:{}:256",
        shot.name, shot.bytes, shot.modified_ns
    )
}
fn metadata(shot: &api::Screenshot) -> String {
    format!(
        "{} · {:.1} KB",
        aurora_launcher_lib::screenshots::date(shot.modified_ns),
        shot.bytes as f64 / 1024.
    )
}
fn error(e: api::ApiError) -> String {
    e.message
}

impl Screenshots {
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        cx.spawn(async move |weak, cx| {
            while let Ok(reply) = rx.recv().await {
                if weak
                    .update(cx, |this: &mut Self, cx| {
                        this.receive(reply, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            host,
            runtime,
            tx,
            state: GalleryState::default(),
            loading: false,
            thumbnails_loading: false,
            cache: ArtworkCache::new(36),
            preview: None,
            preview_serial: 0,
            preview_inflight: false,
            preview_error: String::new(),
            message: String::new(),
            rename: cx.new(|cx| InputState::new(window, cx).placeholder("Screenshot filename")),
            work: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn open(&mut self, instance: Option<String>, cx: &mut Context<Self>) {
        self.state.reset(instance);
        self.cache.clear();
        self.preview = None;
        self.message.clear();
        self.refresh(cx);
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.state.busy {
            return;
        }
        self.state.generation += 1;
        self.state.select(None);
        self.state.page = 0;
        self.preview = None;
        self.state.shots.clear();
        // Retry transient read/decoder failures on an explicit refresh, even
        // when a writer kept the same length and modification timestamp.
        self.cache.clear();
        self.work.fetch_add(1, Ordering::Relaxed);
        self.loading = false;
        self.thumbnails_loading = false;
        let Some((host, instance)) = self.host.clone().zip(self.state.instance.clone()) else {
            cx.notify();
            return;
        };
        self.loading = true;
        let generation = self.state.generation;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let result = guarded(api::screenshots(host, instance)).await;
            let _ = tx.send(Reply::Listed(generation, result)).await;
        });
        cx.notify();
    }
    fn load_page(&mut self, cx: &mut Context<Self>) {
        let Some((host, instance)) = self.host.clone().zip(self.state.instance.clone()) else {
            return;
        };
        let page = self.state.page;
        let generation = self.state.generation;
        let shots: Vec<_> = self.state.shots[self.state.range()]
            .iter()
            .filter(|shot| !self.cache.contains(&key(&instance, shot)))
            .cloned()
            .collect();
        let work = self.work.clone();
        let ticket = work.fetch_add(1, Ordering::Relaxed) + 1;
        self.thumbnails_loading = !shots.is_empty();
        let tx = self.tx.clone();
        if !shots.is_empty() {
            self.runtime.spawn(async move {
                let mut results = Vec::new();
                for shot in shots {
                    if work.load(Ordering::Relaxed) != ticket {
                        return;
                    }
                    let result = guarded(api::screenshot_image(
                        host.clone(),
                        instance.clone(),
                        shot.clone(),
                        false,
                    ))
                    .await;
                    results.push((shot, result));
                }
                let _ = tx.send(Reply::Thumbnails(generation, page, results)).await;
            });
        }
        cx.notify();
    }
    fn select(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.busy {
            return;
        }
        self.state.select(index);
        self.preview = None;
        self.preview_error.clear();
        self.preview_serial += 1;
        let Some(shot) = self.state.current().cloned() else {
            cx.notify();
            return;
        };
        self.rename.update(cx, |input, cx| {
            input.set_value(shot.name.clone(), window, cx)
        });
        self.request_preview();
        cx.notify();
    }
    fn request_preview(&mut self) {
        // Coalesce rapid navigation: one preview in flight, then only the
        // latest requested selection. Never queue a decode for every click.
        if self.preview_inflight {
            return;
        }
        let Some(shot) = self.state.current().cloned() else {
            return;
        };
        let Some((host, instance)) = self.host.clone().zip(self.state.instance.clone()) else {
            return;
        };
        self.preview_inflight = true;
        let generation = self.state.generation;
        let serial = self.preview_serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let result = guarded(api::screenshot_image(host, instance, shot, true)).await;
            let _ = tx.send(Reply::Preview(generation, serial, result)).await;
        });
    }
    fn run(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.state.busy {
            return;
        }
        let shot = if action == "delete" {
            self.state.confirm_delete()
        } else {
            self.state.current().cloned()
        };
        let Some(shot) = shot else {
            return;
        };
        let Some((host, instance)) = self.host.clone().zip(self.state.instance.clone()) else {
            return;
        };
        let name = self.rename.read(cx).value().to_string();
        let generation = self.state.generation;
        self.state.busy = true;
        self.message.clear();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let result = if action == "rename" {
                guarded(api::screenshot_rename(host, instance, shot, name)).await
            } else {
                guarded(api::screenshot_action(
                    host,
                    instance,
                    shot,
                    action.into(),
                    action == "delete",
                ))
                .await
            };
            let _ = tx.send(Reply::Action(generation, action, result)).await;
        });
        cx.notify();
    }
    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Listed(generation, result) if self.state.accepts(generation) => {
                self.loading = false;
                match result {
                    Ok(shots) => {
                        self.state.shots = shots;
                        let instance = self.state.instance.as_deref().unwrap_or("");
                        self.cache.retain(|old| {
                            self.state.shots.iter().any(|s| key(instance, s) == *old)
                        });
                        self.load_page(cx);
                    }
                    Err(e) => self.message = error(e),
                }
            }
            Reply::Thumbnails(generation, page, results)
                if self.state.accepts(generation) && self.state.page == page =>
            {
                self.thumbnails_loading = false;
                for (shot, result) in results {
                    let image = result
                        .ok()
                        .and_then(|b| images::rgba_image(b.width, b.height, b.rgba));
                    self.cache.insert(
                        key(self.state.instance.as_deref().unwrap_or(""), &shot),
                        image,
                        |_| false,
                    );
                }
            }
            Reply::Preview(generation, serial, result) => {
                self.preview_inflight = false;
                if self.state.accepts(generation)
                    && serial == self.preview_serial
                    && self.state.current().is_some()
                {
                    match result {
                        Ok(b) => {
                            self.preview = images::rgba_image(b.width, b.height, b.rgba);
                            if self.preview.is_none() {
                                self.preview_error = "This image cannot be displayed.".into();
                            }
                        }
                        Err(e) => self.preview_error = error(e),
                    }
                } else {
                    self.request_preview();
                }
            }
            Reply::Action(generation, action, result) if self.state.accepts(generation) => {
                self.state.busy = false;
                self.state.delete = None;
                match result {
                    Ok(()) => {
                        self.message = match action {
                            "copyImage" => "Image copied.",
                            "copyFile" => "File copied.",
                            "reveal" => "Screenshot revealed in Explorer.",
                            "delete" => "Screenshot moved to the Recycle Bin.",
                            _ => "Screenshot renamed.",
                        }
                        .into();
                        if matches!(action, "delete" | "rename") {
                            self.refresh(cx);
                        }
                    }
                    Err(e) => self.message = error(e),
                }
            }
            _ => {}
        }
    }
}

impl Render for Screenshots {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.cache.flush_dropped(window);
        let t = theme::theme(cx).clone();
        let busy = self.state.busy;
        self.rename
            .update(cx, |input, cx| input.set_disabled(busy, cx));
        let mut root = ui::card(&t).gap(px(12.)).child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(ui::caption(
                    &t,
                    match self.state.shots.len() {
                        1 => "1 screenshot".to_owned(),
                        count => format!("{count} screenshots"),
                    },
                ))
                .child(
                    AuroraButton::new("screenshots-refresh", "Refresh")
                        .size(ButtonSize::Small)
                        .icon("refresh")
                        .disabled(busy || self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                ),
        );
        if !self.message.is_empty() {
            root = root.child(ui::caption(&t, self.message.clone()));
        }
        if self.loading {
            root = root.child(ui::caption(&t, "Loading screenshots…"));
        } else if self.state.shots.is_empty() {
            root = root.child(
                ui::empty_state(
                    &t,
                    "photo_library",
                    "No screenshots yet",
                    "Press F2 in game to take a screenshot. It will appear here after Refresh.",
                )
                .bg(gpui::transparent_black())
                .border_0()
                .py(px(28.)),
            );
        } else {
            // The gallery fills the detail page inside the card padding.
            let available = (f32::from(window.viewport_size().width)
                - crate::navigation::PAGE_CHROME
                - 2. * ui::CARD_PADDING
                - 2.)
                .max(196.);
            let columns = (available / 212.).floor().max(1.) as usize;
            let width = ((available - (columns - 1) as f32 * 12.) / columns as f32).min(320.);
            let mut grid = div().flex().flex_wrap().gap(px(12.)).w_full();
            for index in self.state.range() {
                let shot = &self.state.shots[index];
                let image = self
                    .cache
                    .image(&key(self.state.instance.as_deref().unwrap_or(""), shot));
                let label = format!("Open {}. {}", shot.name, metadata(shot));
                let tile = div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .w_full()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(px(width * 0.5625))
                            .bg(t.sunken)
                            .rounded(t.radius)
                            .overflow_hidden()
                            .child(if let Some(image) = image {
                                fitted_image(image, width - 18., width * 0.5625).into_any_element()
                            } else {
                                div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(t.muted_foreground)
                                    .child(if self.thumbnails_loading {
                                        "Loading…"
                                    } else {
                                        "Preview unavailable"
                                    })
                                    .into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_size(px(ui::TEXT_BODY))
                            // Truncation clips to one line box; leave room for descenders.
                            .line_height(px(ui::TEXT_BODY + 7.))
                            .text_color(t.foreground)
                            .child(shot.name.clone()),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_size(px(ui::TEXT_CAPTION))
                            .line_height(px(ui::TEXT_CAPTION + 6.))
                            .text_color(t.muted_foreground)
                            .child(metadata(shot)),
                    );
                grid = grid.child(
                    ui::list_button(&t, SharedString::from(format!("screenshot-{index}")), false)
                        .accessibility_label(label)
                        .disabled(busy)
                        .w(px(width))
                        .p(px(8.))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select(Some(index), window, cx)
                        }))
                        .child(tile),
                );
            }
            let page = self.state.page;
            let pages = self.state.shots.len().div_ceil(PAGE_SIZE);
            root = root.child(grid).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("screenshots-previous-page", "Previous page")
                            .size(ButtonSize::Small)
                            .disabled(busy || page == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.page = this.state.page.saturating_sub(1);
                                this.load_page(cx);
                            })),
                    )
                    .child(ui::caption(&t, format!("Page {} of {pages}", page + 1)))
                    .child(
                        AuroraButton::new("screenshots-next-page", "Next page")
                            .size(ButtonSize::Small)
                            .disabled(busy || page + 1 >= pages)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.page += 1;
                                this.load_page(cx);
                            })),
                    ),
            );
        }
        let panel_width = (window.viewport_size().width - px(80.)).min(px(1120.));
        let preview_height = (window.viewport_size().height - px(370.))
            .max(px(100.))
            .min(px(620.));
        let mut panel = div()
            .id("screenshot-preview-panel")
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(20.))
            .bg(t.raised)
            .rounded(t.radius)
            .w(panel_width)
            .max_h(window.viewport_size().height - px(48.))
            .overflow_y_scroll();
        if let Some(shot) = self.state.current().cloned() {
            let index = self.state.selected.unwrap();
            panel =
                panel
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(ui::TEXT_BODY))
                                            .text_color(t.foreground)
                                            .child(shot.name.clone()),
                                    )
                                    .child(ui::caption(&t, metadata(&shot))),
                            )
                            .child(
                                AuroraButton::new("screenshot-close", "Close")
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.select(None, window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(preview_height)
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .overflow_hidden()
                            .bg(t.sunken)
                            .rounded(t.radius)
                            .child(if let Some(image) = self.preview.clone() {
                                fitted_image(
                                    image,
                                    f32::from(panel_width - px(40.)),
                                    f32::from(preview_height),
                                )
                                .into_any_element()
                            } else {
                                div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(t.muted_foreground)
                                    .child(if self.preview_error.is_empty() {
                                        "Loading preview…".to_string()
                                    } else {
                                        self.preview_error.clone()
                                    })
                                    .into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                AuroraButton::new("screenshot-previous", "Previous")
                                    .size(ButtonSize::Small)
                                    .disabled(busy || index == 0)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select(index.checked_sub(1), window, cx)
                                    })),
                            )
                            .child(ui::caption(
                                &t,
                                format!("{} of {}", index + 1, self.state.shots.len()),
                            ))
                            .child(
                                AuroraButton::new("screenshot-next", "Next")
                                    .size(ButtonSize::Small)
                                    .disabled(busy || index + 1 >= self.state.shots.len())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select(Some(index + 1), window, cx)
                                    })),
                            ),
                    );
            let mut actions = div().flex().flex_wrap().gap(px(8.));
            for (id, label) in [
                ("copyImage", "Copy image"),
                ("copyFile", "Copy file"),
                ("reveal", "Reveal in Explorer"),
            ] {
                actions = actions.child(
                    AuroraButton::new(id, label)
                        .size(ButtonSize::Small)
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| this.run(id, cx))),
                );
            }
            panel = panel.child(actions).child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(8.))
                    .child(div().flex_1().min_w_0().child(
                        AuroraInput::new(&self.rename).label("Filename (keep the extension)"),
                    ))
                    .child(
                        AuroraButton::new("screenshot-rename", "Rename")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.run("rename", cx))),
                    ),
            );
            if self.state.delete.is_some() {
                panel = panel.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::caption(
                            &t,
                            format!("Move {} to the Recycle Bin?", shot.name),
                        ))
                        .child(
                            AuroraButton::new("screenshot-delete-cancel", "Cancel")
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.delete = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            AuroraButton::new("screenshot-delete-confirm", "Move to Recycle Bin")
                                .variant(ButtonVariant::Danger)
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.run("delete", cx))),
                        ),
                );
            } else {
                panel = panel.child(
                    AuroraButton::new("screenshot-delete", "Delete…")
                        .variant(ButtonVariant::DangerSubtle)
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.delete = this.state.current().cloned();
                            cx.notify();
                        })),
                );
            }
            if !self.message.is_empty() {
                panel = panel.child(ui::caption(&t, self.message.clone()));
            }
        }
        let view = cx.entity();
        let veto = view.clone();
        root.child(
            div().absolute().size_0().child(
                Dialog::new(cx)
                    .open(self.state.selected.is_some())
                    .accessibility_label("Screenshot preview")
                    .close_on_backdrop_press(false)
                    .on_ok(|_, _, _| false)
                    .on_cancel(move |_, _, cx| !veto.read(cx).state.busy)
                    .on_open_change(move |open, _, window, cx| {
                        if !open {
                            view.update(cx, |this, cx| this.select(None, window, cx));
                        }
                    })
                    .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                    .popup(panel),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn landscape_and_portrait_previews_fit_without_cropping_or_distortion() {
        for (source, bounds) in [
            ((960., 540.), (1000., 390.)),
            ((1080., 1920.), (256., 144.)),
            ((1920., 1080.), (196., 110.)),
        ] {
            let fitted = fitted_size(source, bounds);
            assert!(fitted.0 <= bounds.0 + 0.001 && fitted.1 <= bounds.1 + 0.001);
            assert!((fitted.0 / fitted.1 - source.0 / source.1).abs() < 0.001);
            assert!((fitted.0 - bounds.0).abs() < 0.001 || (fitted.1 - bounds.1).abs() < 0.001);
        }
    }
    fn shot(name: &str) -> api::Screenshot {
        api::Screenshot {
            name: name.into(),
            bytes: 12,
            modified_ns: 123,
        }
    }
    #[test]
    fn deletion_requires_review_of_the_current_snapshot() {
        let mut state = GalleryState {
            shots: vec![shot("a.png"), shot("b.png")],
            ..Default::default()
        };
        state.select(Some(0));
        assert!(state.confirm_delete().is_none());
        state.delete = state.current().cloned();
        assert_eq!(state.confirm_delete(), Some(shot("a.png")));
        state.select(Some(1));
        assert!(state.confirm_delete().is_none());
        state.delete = Some(shot("a.png"));
        assert!(state.confirm_delete().is_none());
        state.reset(Some("another-instance".into()));
        assert!(state.delete.is_none());
        assert!(state.selected.is_none());
    }
    #[test]
    fn pages_are_bounded_and_old_instance_replies_are_ignored() {
        let mut state = GalleryState {
            shots: (0..305).map(|i| shot(&format!("{i}.png"))).collect(),
            ..Default::default()
        };
        assert_eq!(state.range(), 0..12);
        state.page = 25;
        assert_eq!(state.range(), 300..305);
        let generation = state.generation;
        state.reset(Some("new".into()));
        assert!(!state.accepts(generation));
        assert_eq!(state.range(), 0..0);
    }
    #[test]
    fn retained_thumbnail_identity_includes_instance_size_and_modification_time() {
        let original = shot("a.png");
        let mut changed = original.clone();
        changed.bytes += 1;
        assert_ne!(key("one", &original), key("one", &changed));
        changed = original.clone();
        changed.modified_ns += 1;
        assert_ne!(key("one", &original), key("one", &changed));
        assert_ne!(key("one", &original), key("two", &original));
    }
}
