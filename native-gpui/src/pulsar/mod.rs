//! Pulsar presentation. All storage, analytics and OS actions cross the shared API.
mod analysis_ui;
mod charts;
mod compare;
mod detail;
mod navigation;
mod page;
mod playback;
mod projections;
mod sections;
mod tabs;

use crate::{api, backend::guarded};
use aurora_launcher_lib::{host::Host, pulsar::view::*};
use gpui::{Context, Entity, Subscription, Task, Window, prelude::*};
use gpui_kit::base::input::{InputEvent, InputState};
use std::time::Duration;

pub(super) const TOP_TABS: [&str; 8] = [
    "Overview",
    "Offense",
    "Defense",
    "Flow",
    "History",
    "Opponents",
    "Records",
    "Settings",
];
pub(super) const DETAIL_TABS: [&str; 6] = [
    "Summary",
    "Offense",
    "Defense",
    "Flow",
    "Playback",
    "Evidence / Timeline",
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReadToken {
    revision: u64,
    key: Option<FightKey>,
}
fn current(reply: &ReadToken, revision: u64, detail: Option<&DetailRequest>) -> bool {
    reply.revision == revision && reply.key.as_ref() == detail.map(|d| &d.key)
}

enum Reply {
    Library(u64, api::ApiResult<LibraryView>),
    Analysis(u64, api::ApiResult<AnalysisView>),
    Opponents(u64, api::ApiResult<Vec<OpponentView>>),
    Records(u64, api::ApiResult<Vec<RecordView>>),
    Settings(u64, api::ApiResult<SettingsView>),
    Focus(ReadToken, api::ApiResult<FightDetailView>),
    Detail(
        ReadToken,
        api::ApiResult<FightDetailView>,
        api::ApiResult<PlaybackView>,
    ),
    Compare(u64, api::ApiResult<CompareView>),
    Action(api::ApiResult<ActionReceipt>, bool),
}

pub struct Pulsar {
    host: Option<Host>,
    runtime: tokio::runtime::Handle,
    tx: async_channel::Sender<Reply>,
    revision: u64,
    detail_revision: u64,
    focus_revision: u64,
    analysis_revision: u64,
    compare_revision: u64,
    active: bool,
    visible: bool,
    root_focus: gpui::FocusHandle,
    playback_focus: gpui::FocusHandle,
    seek_focus: gpui::FocusHandle,
    history_focus: gpui::FocusHandle,
    history_scroll: gpui::UniformListScrollHandle,
    /// The launcher's scroll container for this page, handed over on each render.
    pub page_scroll: Option<gpui::ScrollHandle>,
    scroll_memory: navigation::ScrollMemory,
    seek_bounds: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// Last rendered viewport width; drives FightList column dropping.
    width: f32,
    dirty: bool,
    loading: bool,
    detail_loading: bool,
    message: String,
    error: Option<String>,
    query: LibraryQuery,
    window: AnalysisWindow,
    weapon: Option<String>,
    timing_result: Option<String>,
    top_tab: usize,
    trend_metric: usize,
    detail_tab: usize,
    /// Selected contextual section per tab index (0 = all sections), kept across tab changes.
    sections: [usize; 4],
    library: LibraryView,
    analysis: Option<AnalysisView>,
    opponents: Vec<OpponentView>,
    records: Vec<RecordView>,
    settings: Option<SettingsView>,
    detail_request: Option<DetailRequest>,
    detail: Option<FightDetailView>,
    focus_request: Option<DetailRequest>,
    focus_detail: Option<FightDetailView>,
    preferences: charts::Preferences,
    jump_span: f64,
    jump_span_slider: Entity<gpui_kit::base::slider::SliderState>,
    jump_start_slider: Entity<gpui_kit::base::slider::SliderState>,
    jump_start: f64,
    metric_info: Option<String>,
    table_expanded: std::collections::BTreeSet<String>,
    playback: Option<PlaybackView>,
    selected: Option<FightKey>,
    compare_keys: Vec<FightKey>,
    comparison: Option<CompareView>,
    compare_open: bool,
    playing: bool,
    speed: f64,
    playhead: f64,
    ticker: Option<Task<()>>,
    event_filter: Option<String>,
    combo_selection: usize,
    show_methods: bool,
    show_data: bool,
    show_evidence: bool,
    aim_density: bool,
    aim_third: usize,
    aim_selection: Option<i64>,
    delete_open: bool,
    delete_source: bool,
    export_open: bool,
    busy: bool,
    search: Entity<InputState>,
    export_path: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
    #[cfg(debug_assertions)]
    showcase_state: Option<String>,
    /// Showcase only: pick the first manual participant once the detail arrives.
    #[cfg(debug_assertions)]
    showcase_participant: bool,
    #[cfg(debug_assertions)]
    showcase_ready: bool,
    #[cfg(debug_assertions)]
    showcase_options: Vec<String>,
}

impl Pulsar {
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
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search opponents or servers…"));
        let export_path = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Destination folder for this report")
        });
        let sub = cx.subscribe(&search, |this: &mut Self, input, _: &InputEvent, cx| {
            let value = input.read(cx).value().to_string();
            if this.query.search != value {
                this.query.search = value;
                this.query.offset = 0;
                this.selected = None;
                this.compare_keys.clear();
                this.load(false, cx);
            }
        });
        let visibility =
            cx.observe_window_visibility(window, |this: &mut Self, visibility, _, cx| {
                this.visible = visibility.is_visible();
                if !this.visible {
                    this.pause();
                }
                cx.notify();
            });
        let jump_span_slider = cx.new(|_| {
            gpui_kit::base::slider::SliderState::new()
                .min(1.)
                .max(100.)
                .step(1.)
                .default_value(100.)
        });
        let jump_start_slider = cx.new(|_| {
            gpui_kit::base::slider::SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(0.)
        });
        let jump_span_sub = cx.subscribe(
            &jump_span_slider,
            |this: &mut Self, _, e: &gpui_kit::base::slider::SliderEvent, cx| {
                let (gpui_kit::base::slider::SliderEvent::Change(v)
                | gpui_kit::base::slider::SliderEvent::Release(v)) = e;
                this.jump_span = v.end() as f64;
                cx.notify();
            },
        );
        let jump_start_sub = cx.subscribe(
            &jump_start_slider,
            |this: &mut Self, _, e: &gpui_kit::base::slider::SliderEvent, cx| {
                let (gpui_kit::base::slider::SliderEvent::Change(v)
                | gpui_kit::base::slider::SliderEvent::Release(v)) = e;
                this.jump_start = v.end() as f64;
                cx.notify();
            },
        );
        let mut this = Self {
            host,
            runtime,
            tx,
            revision: 0,
            detail_revision: 0,
            focus_revision: 0,
            analysis_revision: 0,
            compare_revision: 0,
            active: true,
            visible: window.is_visible(),
            root_focus: cx.focus_handle(),
            playback_focus: cx.focus_handle(),
            seek_focus: cx.focus_handle(),
            history_focus: cx.focus_handle(),
            history_scroll: gpui::UniformListScrollHandle::new(),
            page_scroll: None,
            scroll_memory: Default::default(),
            seek_bounds: Default::default(),
            width: window.viewport_size().width.as_f32(),
            dirty: false,
            loading: false,
            detail_loading: false,
            message: String::new(),
            error: None,
            query: LibraryQuery {
                limit: 100,
                ..Default::default()
            },
            window: AnalysisWindow::Last25,
            weapon: None,
            timing_result: None,
            top_tab: 0,
            trend_metric: 0,
            detail_tab: 0,
            sections: [0, 1, 1, 1],
            library: LibraryView::default(),
            analysis: None,
            opponents: vec![],
            records: vec![],
            settings: None,
            detail_request: None,
            detail: None,
            focus_request: None,
            focus_detail: None,
            preferences: Default::default(),
            jump_span: 100.,
            jump_span_slider,
            jump_start_slider,
            jump_start: 0.,
            metric_info: None,
            table_expanded: Default::default(),
            playback: None,
            selected: None,
            compare_keys: vec![],
            comparison: None,
            compare_open: false,
            playing: false,
            speed: 1.,
            playhead: 0.,
            ticker: None,
            event_filter: None,
            combo_selection: 0,
            show_methods: false,
            show_data: false,
            show_evidence: true,
            aim_density: false,
            aim_third: 0,
            aim_selection: None,
            delete_open: false,
            delete_source: false,
            export_open: false,
            busy: false,
            search,
            export_path,
            _subscriptions: vec![sub, visibility, jump_span_sub, jump_start_sub],
            #[cfg(debug_assertions)]
            showcase_state: if crate::showcase() {
                std::env::var("AURORA_SHOWCASE_PULSAR").ok()
            } else {
                None
            },
            #[cfg(debug_assertions)]
            showcase_participant: false,
            #[cfg(debug_assertions)]
            showcase_ready: false,
            #[cfg(debug_assertions)]
            showcase_options: Vec::new(),
        };
        #[cfg(debug_assertions)]
        if let Some(state) = this.showcase_state.as_deref() {
            let state = state.to_owned();
            let mut parts = state.split('@');
            let state = parts.next().unwrap_or_default();
            this.showcase_options = parts.map(str::to_owned).collect();
            this.showcase_state = Some(state.to_owned());
            // `<tab>[:<section>]`, e.g. `offense:aim`.
            let (tab, section) = state.split_once(':').unwrap_or((state, ""));
            this.top_tab = TOP_TABS
                .iter()
                .position(|t| t.eq_ignore_ascii_case(tab))
                .unwrap_or(if state.starts_with("compare") { 4 } else { 0 });
            if let Some(i) = sections::for_tab(this.top_tab)
                .iter()
                .position(|(label, _)| {
                    label.to_lowercase().starts_with(section) && !section.is_empty()
                })
            {
                this.sections[this.top_tab] = i + 1;
            }
        }
        this.root_focus.focus(window, cx);
        this.refresh(cx);
        this
    }
    /// True while a text field owns the keyboard, so page shortcuts stay out of its way.
    pub fn typing(&self, window: &Window, cx: &gpui::App) -> bool {
        [&self.search, &self.export_path].into_iter().any(|input| {
            input
                .read(cx)
                .presentation()
                .focus_handle()
                .is_focused(window)
        })
    }
    /// Page-level keys (P4): digits pick a tab, Escape closes the innermost view.
    fn on_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.typing(window, cx) {
            return false;
        }
        if key == "escape" {
            if self.delete_open || self.export_open {
                if !self.busy {
                    self.delete_open = false;
                    self.export_open = false;
                }
            } else if self.compare_open {
                self.compare_open = false;
                self.top_tab = 4;
            } else if self.detail_request.is_some() {
                self.close_detail();
            } else {
                return false;
            }
            self.root_focus.focus(window, cx);
            cx.notify();
            return true;
        }
        let Some(digit) = key.parse::<usize>().ok().filter(|d| *d >= 1) else {
            return false;
        };
        if self.compare_open || self.delete_open || self.export_open {
            return false;
        }
        if self.detail_request.is_some() {
            if digit > DETAIL_TABS.len() {
                return false;
            }
            self.pause();
            self.detail_tab = digit - 1;
            if self.detail_tab == 4 {
                self.playback_focus.focus(window, cx);
            }
        } else {
            if digit > TOP_TABS.len() {
                return false;
            }
            self.top_tab = digit - 1;
            self.load_sections();
        }
        cx.notify();
        true
    }
    pub fn show(&mut self, instance: String, cx: &mut Context<Self>) {
        self.query.instance_id = Some(instance);
        self.query.offset = 0;
        self.close_detail();
        self.load(false, cx);
    }
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.active = active;
        if !active {
            self.pause();
        } else if self.dirty {
            self.refresh(cx);
        }
        cx.notify();
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load(true, cx);
    }
    pub fn on_host_event(
        &mut self,
        name: &str,
        payload: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if name == "pulsar-changed" {
            // Refresh emits this event itself. Read the retained index here;
            // importing again would echo indefinitely and invalidate every reply.
            if self.active && !self.busy {
                self.load(false, cx);
            } else {
                self.dirty = true;
            }
        } else if name == "launch-state"
            && matches!(payload["status"].as_str(), Some("exited" | "failed"))
        {
            if self.active && !self.busy {
                self.refresh(cx);
            } else {
                self.dirty = true;
            }
        }
    }

    fn load(&mut self, import: bool, cx: &mut Context<Self>) {
        self.revision = self.revision.wrapping_add(1);
        self.analysis_revision = self.analysis_revision.wrapping_add(1);
        self.dirty = false;
        self.loading = true;
        self.error = None;
        self.analysis = None;
        self.settings = None;
        self.focus_revision = self.focus_revision.wrapping_add(1);
        self.focus_request = None;
        self.focus_detail = None;
        self.opponents.clear();
        self.records.clear();
        #[cfg(debug_assertions)]
        if self.showcase_state.as_deref() == Some("loading") {
            // Hold the first read open so the loading state can be captured.
            eprintln!("[pulsar-showcase-ready] loading");
            return;
        }
        #[cfg(debug_assertions)]
        if matches!(self.showcase_state.as_deref(), Some("empty" | "error")) {
            self.loading = false;
            self.library = LibraryView::default();
            if self.showcase_state.as_deref() == Some("error") {
                self.error=Some("This recording could not be read. The source file was preserved. Refresh after the recording has finished saving.".into());
            }
            eprintln!("[pulsar-showcase-ready] fixture");
            return;
        }
        let Some(host) = self.host.clone() else {
            self.loading = false;
            self.error=Some("Pulsar needs a launcher library. Open the launcher with its data root to browse recordings.".into());
            return;
        };
        let revision = self.revision;
        let query = self.query.clone();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = if import {
                guarded(api::pulsar_refresh(host, query)).await
            } else {
                guarded(api::pulsar_library(host, query)).await
            };
            let _ = tx.send(Reply::Library(revision, r)).await;
        });
        cx.notify();
    }
    fn load_sections(&mut self) {
        self.analysis_revision = self.analysis_revision.wrapping_add(1);
        let Some(host) = self.host.clone() else {
            return;
        };
        let rev = self.analysis_revision;
        let tx = self.tx.clone();
        let query = self.query.clone();
        let req = AnalysisRequest {
            query: query.clone(),
            window: self.window.clone(),
            weapon: self.weapon.clone(),
            timing_result: self.timing_result.clone(),
            focus: self.selected.clone(),
        };
        self.runtime.spawn(async move {
            let r = guarded(api::pulsar_analysis(host, req)).await;
            let _ = tx.send(Reply::Analysis(rev, r)).await;
        });
        if self.top_tab == 5 {
            let host = self.host.clone().unwrap();
            let tx = self.tx.clone();
            let q = query.clone();
            self.runtime.spawn(async move {
                let r = guarded(api::pulsar_opponents(host, q)).await;
                let _ = tx.send(Reply::Opponents(rev, r)).await;
            });
        }
        if self.top_tab == 6 {
            let host = self.host.clone().unwrap();
            let tx = self.tx.clone();
            let q = query.clone();
            self.runtime.spawn(async move {
                let r = guarded(api::pulsar_records(host, q)).await;
                let _ = tx.send(Reply::Records(rev, r)).await;
            });
        }
        {
            if let Some(instance_id) = query.instance_id.or_else(|| {
                self.library
                    .instances
                    .first()
                    .map(|i| i.instance_id.clone())
            }) {
                let host = self.host.clone().unwrap();
                let tx = self.tx.clone();
                self.runtime.spawn(async move {
                    let r = guarded(api::pulsar_settings(
                        host,
                        FolderRequest {
                            instance_id,
                            kind: FolderKind::Recordings,
                        },
                    ))
                    .await;
                    let _ = tx.send(Reply::Settings(rev, r)).await;
                });
            }
        }
    }
    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Library(rev, r) if rev == self.revision => {
                self.loading = false;
                match r {
                    Ok(v) => {
                        self.library = v;
                        if self
                            .selected
                            .as_ref()
                            .is_some_and(|s| !self.library.rows.iter().any(|r| &r.key == s))
                        {
                            self.selected = None;
                        }
                        self.compare_keys
                            .retain(|k| self.library.rows.iter().any(|r| &r.key == k));
                        self.load_sections();
                        #[cfg(debug_assertions)]
                        self.apply_showcase(cx);
                    }
                    Err(e) => self.error = Some(e.message),
                }
            }
            Reply::Analysis(rev, r) if rev == self.analysis_revision => match r {
                Ok(v) => {
                    let focus = v.focus.clone();
                    self.analysis = Some(v);
                    if matches!(self.top_tab, 1 | 3) {
                        if let Some(key) = focus {
                            let request = self
                                .focus_request
                                .clone()
                                .filter(|r| r.key == key)
                                .unwrap_or(DetailRequest {
                                    key,
                                    ..Default::default()
                                });
                            self.request_focus(request, cx);
                        }
                    }
                }
                Err(e) => self.error = Some(e.message),
            },
            Reply::Opponents(rev, r) if rev == self.analysis_revision => match r {
                Ok(v) => self.opponents = v,
                Err(e) => self.error = Some(e.message),
            },
            Reply::Records(rev, r) if rev == self.analysis_revision => match r {
                Ok(v) => self.records = v,
                Err(e) => self.error = Some(e.message),
            },
            Reply::Settings(rev, r) if rev == self.analysis_revision => match r {
                Ok(v) => {
                    self.preferences = charts::Preferences::from_settings(&v);
                    if self.preferences.reduced_motion {
                        self.pause();
                    }
                    self.settings = Some(v);
                }
                Err(e) => self.error = Some(e.message),
            },
            Reply::Focus(token, r)
                if current(&token, self.focus_revision, self.focus_request.as_ref()) =>
            {
                match r {
                    Ok(v) => self.focus_detail = Some(v),
                    Err(e) => self.error = Some(e.message),
                }
            }
            Reply::Detail(token, r, p)
                if current(&token, self.detail_revision, self.detail_request.as_ref()) =>
            {
                self.detail_loading = false;
                match r {
                    Ok(v) => {
                        #[cfg(debug_assertions)]
                        let participant = std::mem::take(&mut self.showcase_participant)
                            .then(|| v.participants.first().map(|p| p.id.clone()))
                            .flatten();
                        self.detail = Some(v);
                        self.playback = p.ok();
                        #[cfg(debug_assertions)]
                        if let (Some(id), Some(mut r)) = (participant, self.detail_request.clone())
                        {
                            let tab = self.detail_tab;
                            r.participant = Some(id);
                            r.exchange_id = None;
                            self.request_detail(r, cx);
                            self.detail_tab = tab;
                        }
                    }
                    Err(e) => self.error = Some(e.message),
                }
            }
            Reply::Compare(rev, r) if rev == self.compare_revision && self.compare_open => {
                match r {
                    Ok(v) => self.comparison = Some(v),
                    Err(e) => self.error = Some(e.message),
                }
            }
            Reply::Action(r, refresh) => {
                self.busy = false;
                match r {
                    Ok(v) => {
                        self.message = v.message;
                        if refresh {
                            self.close_detail();
                            self.refresh(cx);
                        }
                    }
                    Err(e) => self.error = Some(e.message),
                }
            }
            _ => {}
        }
        #[cfg(debug_assertions)]
        self.report_showcase_ready();
    }
    fn open_detail(&mut self, key: FightKey, cx: &mut Context<Self>) {
        self.selected = Some(key.clone());
        self.detail_tab = 0;
        self.request_detail(
            DetailRequest {
                key,
                ..Default::default()
            },
            cx,
        );
    }
    fn request_detail(&mut self, request: DetailRequest, cx: &mut Context<Self>) {
        self.pause();
        self.playhead = 0.;
        self.detail = None;
        self.playback = None;
        self.error = None;
        self.detail_loading = true;
        self.combo_selection = 0;
        self.aim_selection = None;
        self.detail_revision = self.detail_revision.wrapping_add(1);
        self.detail_request = Some(request.clone());
        let token = ReadToken {
            revision: self.detail_revision,
            key: Some(request.key.clone()),
        };
        if let Some(host) = self.host.clone() {
            let tx = self.tx.clone();
            let filters = DetailAnalysisRequest {
                detail: request.clone(),
                weapon: self.weapon.clone(),
                timing_result: self.timing_result.clone(),
            };
            self.runtime.spawn(async move {
                let detail = guarded(api::pulsar_detail_analysis(host.clone(), filters)).await;
                let playback = guarded(api::pulsar_playback(
                    host,
                    PlaybackRequest {
                        detail: request,
                        playhead_seconds: 0.,
                        event_filter: None,
                    },
                ))
                .await;
                let _ = tx.send(Reply::Detail(token, detail, playback)).await;
            });
        }
        cx.notify();
    }
    fn request_focus(&mut self, request: DetailRequest, cx: &mut Context<Self>) {
        self.focus_revision = self.focus_revision.wrapping_add(1);
        self.combo_selection = 0;
        self.focus_detail = None;
        self.focus_request = Some(request.clone());
        let token = ReadToken {
            revision: self.focus_revision,
            key: Some(request.key.clone()),
        };
        if let Some(host) = self.host.clone() {
            let tx = self.tx.clone();
            let filters = DetailAnalysisRequest {
                detail: request,
                weapon: self.weapon.clone(),
                timing_result: self.timing_result.clone(),
            };
            self.runtime.spawn(async move {
                let result = guarded(api::pulsar_detail_analysis(host, filters)).await;
                let _ = tx.send(Reply::Focus(token, result)).await;
            });
        }
        cx.notify();
    }
    fn active_detail(&self) -> Option<&FightDetailView> {
        if self.detail_request.is_some() {
            self.detail.as_ref()
        } else {
            self.focus_detail.as_ref()
        }
    }
    fn select_scope(&mut self, request: DetailRequest, cx: &mut Context<Self>) {
        if self.detail_request.is_some() {
            self.request_detail(request, cx);
        } else {
            self.request_focus(request, cx);
        }
    }
    fn close_detail(&mut self) {
        self.pause();
        self.detail_revision = self.detail_revision.wrapping_add(1);
        self.detail_request = None;
        self.detail = None;
        self.playback = None;
        self.detail_loading = false;
        self.compare_open = false;
        self.comparison = None;
        self.delete_open = false;
        self.export_open = false;
    }
    fn pause(&mut self) {
        self.playing = false;
        self.ticker = None;
    }
    fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if self.preferences.reduced_motion {
            self.step(true, cx);
            return;
        }
        if self.playing {
            self.pause();
            return;
        }
        let duration = self.duration();
        if duration <= 0. || !self.active || !self.visible {
            return;
        }
        if self.playhead >= duration {
            self.playhead = 0.;
        }
        self.playing = true;
        self.ticker = Some(cx.spawn(async move |weak, cx| {
            let mut last = std::time::Instant::now();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let now = std::time::Instant::now();
                let elapsed = now.duration_since(last).as_secs_f64();
                last = now;
                let keep = weak
                    .update(cx, |this, cx| {
                        if !this.playing || !this.active || !this.visible || this.detail_tab != 4 {
                            return false;
                        }
                        this.playhead =
                            playback::advance(this.playhead, elapsed, this.speed, this.duration());
                        if this.playhead >= this.duration() {
                            this.playing = false;
                        }
                        cx.notify();
                        this.playing
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
        cx.notify();
    }
    fn analysis_duration(&self) -> f64 {
        self.active_detail()
            .map(|d| d.summary.duration_nanos as f64 / 1e9)
            .unwrap_or(0.)
    }
    fn duration(&self) -> f64 {
        self.playback
            .as_ref()
            .map(|p| p.duration_seconds)
            .or_else(|| {
                self.detail
                    .as_ref()
                    .map(|d| d.summary.duration_nanos as f64 / 1e9)
            })
            .unwrap_or(0.)
    }
    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.pause();
        if let Some(p) = &self.playback {
            self.playhead = playback::step(&p.events, self.playhead, forward, self.duration());
        }
        cx.notify();
    }
    fn folder(&mut self, kind: FolderKind, cx: &mut Context<Self>) {
        let instance = self
            .detail_request
            .as_ref()
            .map(|r| r.key.instance_id.clone())
            .or_else(|| self.query.instance_id.clone());
        if let (Some(host), Some(instance_id)) = (self.host.clone(), instance) {
            let tx = self.tx.clone();
            self.runtime.spawn(async move {
                let r = guarded(api::pulsar_open_folder(
                    host,
                    FolderRequest { instance_id, kind },
                ))
                .await;
                let _ = tx.send(Reply::Action(r, false)).await;
            });
        } else {
            self.message = "Select an instance to open its folders.".into();
        }
        cx.notify();
    }
    fn view_report(&mut self, key: FightKey) {
        if let Some(host) = self.host.clone() {
            let tx = self.tx.clone();
            self.runtime.spawn(async move {
                let r = guarded(api::pulsar_view_report(host, key)).await;
                let _ = tx.send(Reply::Action(r, false)).await;
            });
        }
    }
    #[cfg(debug_assertions)]
    fn report_showcase_ready(&mut self) {
        if self.showcase_ready
            || !crate::showcase()
            || self.loading
            || self.showcase_state.is_some()
            || self.detail_loading
        {
            return;
        }
        let ready = if self.compare_open {
            self.comparison.is_some()
        } else if self.detail_request.is_some() {
            self.detail.is_some() && (self.detail_tab != 4 || self.playback.is_some())
        } else {
            self.analysis.is_some()
                && match self.top_tab {
                    1 if self.sections[1] == 3 => self.focus_detail.is_some(),
                    3 => self.focus_detail.is_some(),
                    5 => !self.opponents.is_empty(),
                    6 => !self.records.is_empty(),
                    7 => self.settings.is_some(),
                    _ => true,
                }
        };
        if ready {
            for option in self.showcase_options.clone() {
                match option.as_str() {
                    "delete" => self.delete_open = true,
                    "export" => self.export_open = true,
                    "density" => self.aim_density = true,
                    "methods" => self.show_methods = true,
                    "data" => self.show_data = true,
                    "tables" => {
                        if let Some(d) = self.active_detail() {
                            let ids = d
                                .summary_tab
                                .tables
                                .iter()
                                .chain(&d.offense.tables)
                                .chain(&d.defense.tables)
                                .chain(&d.flow.tables)
                                .map(|t| t.id.clone())
                                .collect::<Vec<_>>();
                            self.table_expanded.extend(ids);
                        }
                    }
                    "info" => self.metric_info = Some("accuracy".into()),
                    "opponent" => {
                        if let Some(d) = self.active_detail() {
                            if let Some(i) = d.derived["combos"]["streaks"]
                                .as_array()
                                .and_then(|s| s.iter().position(|s| s["side"] == "OPPONENT"))
                            {
                                self.combo_selection = i;
                            }
                        }
                    }
                    "mid" => self.playhead = self.duration() / 2.,
                    _ => {}
                }
            }
            self.showcase_ready = true;
            eprintln!("[pulsar-showcase-ready] loaded");
        }
    }
    #[cfg(debug_assertions)]
    fn apply_showcase(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.showcase_state.take() else {
            return;
        };
        if let Some(rest) = state.strip_prefix("detail:") {
            // `detail:<id>[:<tab>[:participant]]`
            let mut parts = rest.split(':');
            let id = parts.next().unwrap_or_default();
            let tab = parts.next().unwrap_or("summary");
            let section = parts.next().unwrap_or_default();
            self.showcase_participant = section == "participant";
            if let Some(row) = self
                .library
                .rows
                .iter()
                .find(|r| r.key.fight_id == id || r.key.fight_id.starts_with(id))
                .cloned()
            {
                self.open_detail(row.key, cx);
                self.detail_tab = DETAIL_TABS
                    .iter()
                    .position(|t| {
                        t.eq_ignore_ascii_case(tab)
                            || (tab == "evidence" && t.starts_with("Evidence"))
                    })
                    .unwrap_or(0);
                if let Some(i) = sections::for_tab(self.detail_tab)
                    .iter()
                    .position(|(label, _)| {
                        !section.is_empty() && label.to_lowercase().starts_with(section)
                    })
                {
                    self.sections[self.detail_tab] = i + 1;
                }
            }
        } else if let Some(rest) = state.strip_prefix("compare") {
            let ids: Vec<String> = match rest.strip_prefix(':') {
                Some(list) => list.split(',').map(str::to_owned).collect(),
                // Plain `compare` pairs the two newest recordings in scope.
                None => self
                    .library
                    .rows
                    .iter()
                    .take(2)
                    .map(|r| r.key.fight_id.clone())
                    .collect(),
            };
            self.compare_keys = ids
                .iter()
                .filter_map(|id| {
                    self.library
                        .rows
                        .iter()
                        .find(|r| r.key.fight_id == *id || r.key.fight_id.starts_with(id.as_str()))
                        .map(|r| r.key.clone())
                })
                .collect();
            self.open_compare(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_pulsar_symbol_is_in_the_shared_catalog() {
        for name in [
            "arrow_back",
            "arrow_forward",
            "delete",
            "download",
            "folder_open",
            "info",
            "open_in_new",
            "pause",
            "person",
            "play_arrow",
            "refresh",
            "search_off",
            "swords",
        ] {
            assert!(crate::symbols::path(name).is_some(), "{name}");
        }
    }
    #[test]
    fn stale_reply_rejected_after_scope_and_selection_changes() {
        let key = FightKey {
            instance_id: "editor".into(),
            fight_id: "fight".into(),
        };
        let t = ReadToken {
            revision: 4,
            key: Some(key.clone()),
        };
        let r = DetailRequest {
            key,
            ..Default::default()
        };
        assert!(current(&t, 4, Some(&r)));
        assert!(!current(&t, 5, Some(&r)));
        assert!(!current(&t, 4, None));
        let other = DetailRequest {
            key: FightKey {
                instance_id: "other".into(),
                fight_id: "fight".into(),
            },
            ..Default::default()
        };
        assert!(!current(&t, 4, Some(&other)));
    }
}
