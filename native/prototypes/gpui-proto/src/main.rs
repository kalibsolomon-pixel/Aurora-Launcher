#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! NATIVE-0 GPUI prototype: same content as the Slint prototype where GPUI
//! 0.2 provides primitives. GPUI ships no text field, slider or dropdown;
//! those are noted rather than hand-built for this bounded comparison.

use std::time::Duration;

use gpui::{
    App, Application, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    rgba, size,
};

#[derive(Clone, Copy)]
struct Palette {
    background: u32,
    sidebar: u32,
    surface: u32,
    raised: u32,
    hover: u32,
}

const THEMES: [Palette; 3] = [
    Palette { background: 0x101116, sidebar: 0x0a0b0e, surface: 0x16171d, raised: 0x1d1f27, hover: 0x242631 },
    Palette { background: 0x0d1017, sidebar: 0x070910, surface: 0x121621, raised: 0x191e2b, hover: 0x202636 },
    Palette { background: 0x000000, sidebar: 0x000000, surface: 0x0b0b0e, raised: 0x141419, hover: 0x1b1b22 },
];
const TEXT: u32 = 0xedf1fb;
const MUTED: u32 = 0x929db4;
const ACCENT: u32 = 0x6f5df2;

struct Launcher {
    page: usize,
    theme: usize,
    selected: usize,
    busy: bool,
    progress: f32,
    status: SharedString,
    dialog: bool,
    reduce_motion: bool,
    instances: Vec<(SharedString, SharedString, bool)>,
    rows: Vec<(SharedString, SharedString, bool)>,
}

impl Launcher {
    fn new() -> Self {
        Self {
            page: 0,
            theme: 0,
            selected: 0,
            busy: false,
            progress: 0.0,
            status: "".into(),
            dialog: false,
            reduce_motion: false,
            instances: vec![
                ("Aurora 3.0".into(), "Minecraft 1.21.10 · Fabric 0.17.3".into(), true),
                ("Survival".into(), "Minecraft 1.21.8 · Fabric 0.16.14".into(), true),
                ("Creative build".into(), "Minecraft 1.21.10 · Vanilla".into(), false),
                ("Modpack test".into(), "Minecraft 1.20.1 · Forge 47.4".into(), true),
            ],
            rows: (0..300)
                .map(|i| {
                    (format!("Content item {i:03}").into(), format!("version 1.{i}.0 · author {}", i % 17).into(), i % 3 != 0)
                })
                .collect(),
        }
    }

    fn play(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.status = "Downloading libraries".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            for step in 1..=30u32 {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let gone = this
                    .update(cx, |this, cx| {
                        this.progress = step as f32 / 30.0;
                        this.status = format!("Downloading libraries {step}/30").into();
                        if step == 30 {
                            this.busy = false;
                            this.status = "Ready".into();
                        }
                        cx.notify();
                    })
                    .is_err();
                if gone {
                    break;
                }
            }
        })
        .detach();
    }

    fn button(&self, id: &'static str, label: &str, primary: bool) -> gpui::Stateful<gpui::Div> {
        let p = THEMES[self.theme];
        div()
            .id(id)
            .px_4()
            .py_2()
            .rounded(px(10.))
            .bg(rgb(if primary { ACCENT } else { p.raised }))
            .hover(move |s| s.bg(rgb(if primary { 0x6f63e8 } else { p.hover })))
            .text_color(rgb(TEXT))
            .cursor_pointer()
            .child(label.to_string())
    }

    fn nav(&self, id: &'static str, label: &'static str, page: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let p = THEMES[self.theme];
        let active = self.page == page;
        div()
            .id(id)
            .h(px(56.))
            .rounded(px(10.))
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(rgb(if active { TEXT } else { MUTED }))
            .when(active, |d| d.bg(rgba(0x8b80ff24)))
            .hover(move |s| s.bg(rgb(p.hover)))
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            }))
    }

    fn toggle(&self, id: impl Into<gpui::ElementId>, on: bool) -> gpui::Stateful<gpui::Div> {
        let p = THEMES[self.theme];
        div()
            .id(id)
            .w(px(40.))
            .h(px(22.))
            .rounded(px(11.))
            .bg(rgb(if on { ACCENT } else { p.hover }))
            .flex()
            .items_center()
            .when(on, |d| d.justify_end())
            .px(px(3.))
            .cursor_pointer()
            .child(div().size(px(16.)).rounded(px(8.)).bg(rgb(TEXT)))
    }

    fn home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = THEMES[self.theme];
        let (name, detail, _) = self.instances[self.selected].clone();
        let tiles: Vec<_> = self
            .instances
            .iter()
            .enumerate()
            .map(|(i, (name, detail, ready))| {
                let selected = i == self.selected;
                div()
                    .id(("tile", i))
                    .h(px(72.))
                    .rounded(px(10.))
                    .p_3()
                    .flex()
                    .gap_3()
                    .items_center()
                    .bg(rgb(if selected { p.raised } else { p.surface }))
                    .when(selected, |d| d.border_1().border_color(rgb(0x8b80ff)))
                    .hover(move |s| s.bg(rgb(p.hover)))
                    .cursor_pointer()
                    .child(div().size(px(48.)).rounded(px(10.)).bg(rgb(p.raised)))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(div().text_sm().text_color(rgb(TEXT)).child(name.clone()))
                            .child(div().text_xs().text_color(rgb(MUTED)).child(detail.clone())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(if *ready { 0x7ee2b8 } else { 0x8fb7ff }))
                            .child(if *ready { "Ready" } else { "Install" }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i;
                        cx.notify();
                    }))
            })
            .collect();
        let mut hero = div()
            .flex_1()
            .min_h(px(220.))
            .rounded(px(14.))
            .bg(rgb(p.surface))
            .p_6()
            .flex()
            .flex_col()
            .justify_end()
            .gap_3()
            .child(div().text_2xl().text_color(rgb(TEXT)).child(name))
            .child(div().text_sm().text_color(rgb(MUTED)).child(detail))
            .child(
                self.button("play", if self.busy { "Installing…" } else { "Play" }, true)
                    .w(px(180.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.busy {
                            this.play(cx)
                        }
                    })),
            );
        if self.busy || !self.status.is_empty() {
            hero = hero.child(div().text_xs().text_color(rgb(MUTED)).child(self.status.clone())).child(
                div()
                    .h(px(4.))
                    .rounded(px(2.))
                    .bg(rgb(p.raised))
                    .child(div().h_full().w(gpui::relative(self.progress)).rounded(px(2.)).bg(rgb(0x8b80ff))),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_6()
            .p_8()
            .size_full()
            .child(div().text_xl().text_color(rgb(TEXT)).child("Home"))
            .child(div().flex().gap_6().child(hero).child(div().w(px(280.)).flex().flex_col().gap_2().children(tiles)))
    }

    fn list(&self) -> impl IntoElement {
        let p = THEMES[self.theme];
        let rows: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, (name, detail, on))| {
                div()
                    .id(("row", i))
                    .h(px(52.))
                    .flex_none()
                    .rounded(px(10.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(rgb(p.surface))
                    .hover(move |s| s.bg(rgb(p.hover)))
                    .child(div().size(px(32.)).rounded(px(6.)).bg(rgb(p.raised)))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(div().text_sm().text_color(rgb(TEXT)).child(name.clone()))
                            .child(div().text_xs().text_color(rgb(MUTED)).child(detail.clone())),
                    )
                    .child(self.toggle(("toggle", i), *on))
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_8()
            .size_full()
            .child(div().text_xl().text_color(rgb(TEXT)).child("Mods"))
            .child(div().id("rows").flex_1().overflow_y_scroll().flex().flex_col().gap(px(2.)).children(rows))
    }

    fn settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = THEMES[self.theme];
        let row = |label: &'static str| {
            div().flex().items_center().justify_between().py_2().child(div().text_sm().text_color(rgb(TEXT)).child(label))
        };
        let themes: Vec<_> = ["Aurora Dark", "Midnight", "OLED"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                self.button(["th0", "th1", "th2"][i], name, i == self.theme).on_click(cx.listener(move |this, _, _, cx| {
                    this.theme = i;
                    cx.notify();
                }))
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_8()
            .child(div().text_xl().text_color(rgb(TEXT)).child("Settings"))
            .child(
                div()
                    .rounded(px(14.))
                    .bg(rgb(p.surface))
                    .p_4()
                    .child(row("Theme").child(div().flex().gap_2().children(themes)))
                    .child(row("Reduce motion").child(self.toggle("rm", self.reduce_motion).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.reduce_motion = !this.reduce_motion;
                            cx.notify();
                        },
                    ))))
                    .child(row("Microsoft account").child(self.button("acct", "Manage", false).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.dialog = true;
                            cx.notify();
                        },
                    )))),
            )
    }
}

impl Render for Launcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = THEMES[self.theme];
        let content = match self.page {
            0 => self.home(cx).into_any_element(),
            1 => self.list().into_any_element(),
            _ => self.settings(cx).into_any_element(),
        };
        let mut root = div()
            .size_full()
            .flex()
            .bg(rgb(p.background))
            .text_color(rgb(TEXT))
            .font_family("Segoe UI")
            .child(
                div()
                    .w(px(96.))
                    .h_full()
                    .bg(rgb(p.sidebar))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.nav("n0", "Home", 0, cx))
                    .child(self.nav("n1", "Mods", 1, cx))
                    .child(self.nav("n2", "Settings", 2, cx)),
            )
            .child(div().flex_1().h_full().child(content));
        if self.dialog {
            root = root.child(
                div().absolute().size_full().bg(rgba(0x000000a0)).flex().items_center().justify_center().child(
                    div()
                        .w(px(440.))
                        .rounded(px(14.))
                        .bg(rgb(p.raised))
                        .p_6()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_lg().child("Accounts"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child("Sign-in happens in your system browser. Aurora never sees your password."),
                        )
                        .child(self.button("close", "Close", false).on_click(cx.listener(|this, _, _, cx| {
                            this.dialog = false;
                            cx.notify();
                        }))),
                ),
            );
        }
        root
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1120.), px(760.)), cx);
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            |_, cx| cx.new(|_| Launcher::new()),
        )
        .expect("open window");
        cx.activate(true);
    });
}
