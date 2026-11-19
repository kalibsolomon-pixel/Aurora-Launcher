//! One native project window. The parent and this window feed the same
//! Downloads queue; only the catalog, reading position and review are local.

use std::collections::HashSet;

use aurora_launcher_lib::modrinth::{ProjectPage, ProjectRelease, VersionChoice, safe_web_link};
use gpui::{
    AnyElement, App, Bounds, Context, Div, ElementId, Entity, FontStyle, FontWeight,
    HighlightStyle, InteractiveText, ObjectFit, Render, RenderImage, ScrollHandle, SharedString,
    StyledText, UnderlineStyle, Window, WindowBounds, WindowOptions, div, img, point, prelude::*,
    px, size,
};

use super::{Discover, KINDS, SelectUi, channel, grouped, loader_name, markdown, size_label};
use crate::{
    api,
    artwork::{self, ArtworkCache, RawArtwork},
    backend::guarded,
    chrome,
    controls::{AuroraButton, AuroraSelect, ButtonSize, ButtonVariant},
    downloads::Downloads,
    theme::{self, AuroraTheme},
    ui,
};
use aurora_launcher_lib::host::Host;
use std::sync::Arc;

pub(super) struct ProjectTarget {
    pub kind: usize,
    pub project_id: String,
    pub title: String,
    pub target: Option<api::InstanceSummary>,
}

pub(super) fn open(
    host: Host,
    runtime: tokio::runtime::Handle,
    downloads: Entity<Downloads>,
    target: ProjectTarget,
    icon: Option<Arc<RenderImage>>,
    cx: &mut App,
) -> Result<(), String> {
    let bounds = Bounds::centered(None, size(px(900.), px(720.)), cx);
    let mut titlebar = chrome::titlebar();
    titlebar.title = Some(format!("{} · Aurora Launcher", target.title).into());
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(480.), px(420.))),
            titlebar: Some(titlebar),
            ..Default::default()
        },
        |window, cx| {
            let review = cx.new(|cx| {
                Discover::with_downloads(
                    Some(host.clone()),
                    runtime.clone(),
                    window,
                    cx,
                    Some(downloads.clone()),
                )
            });
            cx.new(|cx| ProjectWindow::new(host, runtime, downloads, review, target, icon, cx))
        },
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Description,
    Gallery,
    Versions,
    Changelog,
}

const TABS: [(Tab, &str); 4] = [
    (Tab::Description, "Description"),
    (Tab::Gallery, "Gallery"),
    (Tab::Versions, "Versions"),
    (Tab::Changelog, "Changelog"),
];
const MEDIA_CAPACITY: usize = 24;

#[derive(Default)]
struct VersionFilters {
    minecraft: Option<String>,
    loader: Option<String>,
    version: Option<String>,
}

impl VersionFilters {
    fn minecraft_options(releases: &[ProjectRelease]) -> Vec<String> {
        unique(releases.iter().flat_map(|item| &item.version.game_versions))
    }

    fn loader_options(&self, releases: &[ProjectRelease]) -> Vec<String> {
        unique(
            releases
                .iter()
                .filter(|item| self.matches_minecraft(&item.version))
                .flat_map(|item| &item.version.loaders),
        )
    }

    fn matches_minecraft(&self, version: &VersionChoice) -> bool {
        self.minecraft
            .as_ref()
            .is_some_and(|value| version.game_versions.contains(value))
    }

    fn matching<'a>(&self, releases: &'a [ProjectRelease]) -> Vec<&'a ProjectRelease> {
        releases
            .iter()
            .filter(|item| {
                self.matches_minecraft(&item.version)
                    && self
                        .loader
                        .as_ref()
                        .is_some_and(|value| item.version.loaders.contains(value))
            })
            .collect()
    }

    fn choose_minecraft(
        &mut self,
        value: String,
        releases: &[ProjectRelease],
        kind: usize,
        target: Option<&api::InstanceSummary>,
    ) {
        self.minecraft = Some(value);
        self.version = None;
        let options = self.loader_options(releases);
        let preferred = target.map(|instance| match kind {
            1 => "minecraft",
            2 if options
                .iter()
                .any(|option| option == &instance.platform.kind) =>
            {
                instance.platform.kind.as_str()
            }
            2 if matches!(instance.platform.kind.as_str(), "fabric" | "neoforge") => "iris",
            _ => instance.platform.kind.as_str(),
        });
        self.loader = preferred
            .and_then(|loader| options.iter().find(|value| value.as_str() == loader))
            .or_else(|| (options.len() == 1).then(|| &options[0]))
            .cloned();
    }
}

fn unique<'a>(values: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .filter(|value| seen.insert(*value))
        .cloned()
        .collect()
}

fn install_block(
    kind: usize,
    target: Option<&api::InstanceSummary>,
    filters: &VersionFilters,
) -> Option<&'static str> {
    if kind == 3 {
        return None;
    }
    let Some(target) = target else {
        return Some("Choose an instance in Discover to install this version.");
    };
    if filters.minecraft.as_ref() != Some(&target.minecraft_version) {
        return Some("Choose the target instance's Minecraft version to install.");
    }
    if kind == 0 && filters.loader.as_ref() != Some(&target.platform.kind) {
        return Some("Choose the target instance's loader to install.");
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Picker {
    Minecraft = 0,
    Loader = 1,
    Version = 2,
    Changelog = 3,
}

enum Message {
    Page(api::ApiResult<(ProjectPage, Vec<markdown::Block>)>),
    Icon(Option<RawArtwork>),
    Media(String, Option<RawArtwork>),
    Changelog(String, api::ApiResult<Vec<markdown::Block>>),
}

struct ProjectWindow {
    host: Host,
    runtime: tokio::runtime::Handle,
    downloads: Entity<Downloads>,
    review: Entity<Discover>,
    _queue_events: gpui::Subscription,
    tx: async_channel::Sender<Message>,
    target: ProjectTarget,
    page: Option<api::ApiResult<ProjectPage>>,
    tab: Tab,
    scroll: ScrollHandle,
    filters: VersionFilters,
    pickers: [SelectUi; 4],
    description: Vec<markdown::Block>,
    gallery_index: usize,
    changelog_version: Option<String>,
    changelog: Option<Result<Vec<markdown::Block>, String>>,
    icon: Option<Arc<RenderImage>>,
    artwork: ArtworkCache<String>,
    pending: HashSet<String>,
}

impl ProjectWindow {
    fn new(
        host: Host,
        runtime: tokio::runtime::Handle,
        downloads: Entity<Downloads>,
        review: Entity<Discover>,
        target: ProjectTarget,
        icon: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        cx.spawn(async move |weak, cx| {
            while let Ok(message) = rx.recv().await {
                if weak
                    .update(cx, |this, cx| this.receive(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let _queue_events = cx.observe(&downloads, |_, _, cx| cx.notify());
        let mut view = Self {
            host,
            runtime,
            downloads,
            review,
            _queue_events,
            tx,
            target,
            page: None,
            tab: Tab::Description,
            scroll: ScrollHandle::new(),
            filters: VersionFilters::default(),
            pickers: std::array::from_fn(|_| SelectUi::new(cx)),
            description: Vec::new(),
            gallery_index: 0,
            changelog_version: None,
            changelog: None,
            icon,
            artwork: ArtworkCache::new(MEDIA_CAPACITY),
            pending: HashSet::new(),
        };
        view.load_page();
        if view.icon.is_none() {
            let (tx, host, project) = (
                view.tx.clone(),
                view.host.clone(),
                view.target.project_id.clone(),
            );
            view.runtime.spawn(async move {
                let result = guarded(api::project_artwork(host, project))
                    .await
                    .ok()
                    .flatten()
                    .and_then(|bytes| artwork::raw_from_png(&bytes));
                let _ = tx.send(Message::Icon(result)).await;
            });
        }
        view
    }

    fn load_page(&mut self) {
        self.page = None;
        let base = self.base();
        let (tx, kind, id) = (
            self.tx.clone(),
            KINDS[self.target.kind],
            self.target.project_id.clone(),
        );
        self.runtime.spawn(async move {
            let result = guarded(async move {
                let mut page = api::project_page(kind, id).await?;
                let text = markdown::parse(&std::mem::take(&mut page.body), &base);
                Ok((page, text))
            })
            .await;
            let _ = tx.send(Message::Page(result)).await;
        });
    }

    fn base(&self) -> String {
        format!("https://modrinth.com/project/{}", self.target.project_id)
    }

    fn page(&self) -> Option<&ProjectPage> {
        self.page.as_ref()?.as_ref().ok()
    }

    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Page(result) => {
                self.page = Some(result.map(|(page, description)| {
                    self.description = description;
                    if let Some(target) = &self.target.target
                        && VersionFilters::minecraft_options(&page.releases)
                            .contains(&target.minecraft_version)
                    {
                        self.filters.choose_minecraft(
                            target.minecraft_version.clone(),
                            &page.releases,
                            self.target.kind,
                            Some(target),
                        );
                    }
                    page
                }));
                if self.tab == Tab::Changelog {
                    self.ensure_changelog();
                }
                self.load_tab_artwork();
            }
            Message::Icon(icon) => self.icon = icon.and_then(RawArtwork::into_image),
            Message::Media(url, raw) => {
                self.pending.remove(&url);
                self.artwork
                    .insert(url, raw.and_then(RawArtwork::into_image), |_| false);
            }
            Message::Changelog(version, result) => {
                if self.changelog_version.as_ref() != Some(&version) {
                    return;
                }
                self.changelog = Some(result.map_err(|error| error.message));
                self.load_tab_artwork();
            }
        }
        cx.notify();
    }

    fn load_media(&mut self, url: String) {
        if !url.starts_with("https://cdn.modrinth.com/data/")
            || self.artwork.contains(&url)
            || self.pending.len() >= MEDIA_CAPACITY
            || !self.pending.insert(url.clone())
        {
            return;
        }
        let (tx, host) = (self.tx.clone(), self.host.clone());
        self.runtime.spawn(async move {
            let request = api::project_image(host, url.clone());
            let image = guarded(async move {
                Ok(request
                    .await
                    .and_then(|bytes| artwork::raw_from_png(&bytes)))
            })
            .await
            .ok()
            .flatten();
            let _ = tx.send(Message::Media(url, image)).await;
        });
    }

    fn load_tab_artwork(&mut self) {
        let urls = match self.tab {
            Tab::Description => media_urls(&self.description),
            Tab::Gallery => self
                .page()
                .and_then(|page| page.gallery.get(self.gallery_index))
                .map(|entry| vec![entry.url.clone()])
                .unwrap_or_default(),
            Tab::Changelog => self
                .changelog
                .as_ref()
                .and_then(|notes| notes.as_ref().ok())
                .map(|notes| media_urls(notes))
                .unwrap_or_default(),
            Tab::Versions => return,
        };
        for url in urls {
            self.load_media(url);
        }
    }

    fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        self.scroll.set_offset(point(px(0.), px(0.)));
        for picker in &mut self.pickers {
            picker.open = false;
        }
        if tab == Tab::Changelog {
            self.ensure_changelog();
        }
        self.load_tab_artwork();
        cx.notify();
    }

    fn ensure_changelog(&mut self) {
        let version = self
            .filters
            .version
            .clone()
            .or_else(|| self.changelog_version.clone())
            .or_else(|| self.page()?.releases.first().map(|r| r.version.id.clone()));
        if let Some(version) = version {
            self.load_changelog(version);
        }
    }

    fn load_changelog(&mut self, version: String) {
        if self.changelog_version.as_ref() == Some(&version)
            && !matches!(self.changelog, Some(Err(_)))
        {
            return;
        }
        self.changelog_version = Some(version.clone());
        self.changelog = None;
        let (tx, project) = (self.tx.clone(), self.target.project_id.clone());
        let base = self.base();
        self.runtime.spawn(async move {
            let request = api::project_changelog(project, version.clone());
            let result = guarded(async move {
                let text = request.await?;
                Ok(markdown::parse(text.as_deref().unwrap_or_default(), &base))
            })
            .await;
            let _ = tx.send(Message::Changelog(version, result)).await;
        });
    }

    fn picker(
        &self,
        picker: Picker,
        label: &'static str,
        choices: Vec<(String, String)>,
        current: Option<&str>,
        width: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let index = picker as usize;
        let select = &self.pickers[index];
        let selected = choices
            .iter()
            .position(|(value, _)| Some(value.as_str()) == current);
        let labels = choices
            .iter()
            .map(|(_, label)| SharedString::from(label.clone()))
            .collect();
        let disabled = choices.is_empty();
        super::field(
            theme::theme(cx),
            label,
            AuroraSelect::new(
                ("project-picker", index),
                label,
                labels,
                &select.trigger,
                &select.content,
                select.bounds.clone(),
            )
            .width(px(width))
            .selected(selected)
            .open(select.open)
            .highlight(select.highlight)
            .disabled(disabled)
            .on_open_change(cx.processor(move |this, open, _, cx| {
                let select = &mut this.pickers[index];
                select.open = open;
                if open {
                    select.highlight = selected.unwrap_or(0);
                }
                cx.notify();
            }))
            .on_highlight(cx.processor(move |this, highlight, _, cx| {
                this.pickers[index].highlight = highlight;
                cx.notify();
            }))
            .on_confirm(cx.processor(move |this, choice: usize, _, cx| {
                this.pickers[index].open = false;
                if let Some((value, _)) = choices.get(choice) {
                    this.choose(picker, value.clone());
                }
                cx.notify();
            })),
        )
    }

    fn choose(&mut self, picker: Picker, value: String) {
        match picker {
            Picker::Minecraft => {
                if let Some(Ok(page)) = &self.page {
                    self.filters.choose_minecraft(
                        value,
                        &page.releases,
                        self.target.kind,
                        self.target.target.as_ref(),
                    );
                }
            }
            Picker::Loader => {
                self.filters.loader = Some(value);
                self.filters.version = None;
            }
            Picker::Version => self.filters.version = Some(value),
            Picker::Changelog => self.load_changelog(value),
        }
    }

    fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.page() else { return };
        let Some(release) = self
            .filters
            .matching(&page.releases)
            .into_iter()
            .find(|item| Some(&item.version.id) == self.filters.version.as_ref())
        else {
            return;
        };
        if release.version.unsupported_reason.is_some()
            || install_block(self.target.kind, self.target.target.as_ref(), &self.filters).is_some()
        {
            return;
        }
        self.review.update(cx, |review, cx| {
            review.review_project_version(&self.target, page, &release.version, window, cx);
        });
    }
}

fn media_urls(blocks: &[markdown::Block]) -> Vec<String> {
    fn visit<'a>(blocks: &'a [markdown::Block], urls: &mut Vec<&'a String>) {
        for block in blocks {
            if urls.len() >= MEDIA_CAPACITY {
                return;
            }
            match block {
                markdown::Block::Image { url: Some(url), .. }
                    if url.starts_with("https://cdn.modrinth.com/data/")
                        && !urls.contains(&url) =>
                {
                    urls.push(url);
                }
                markdown::Block::List(_, children)
                | markdown::Block::Item(children)
                | markdown::Block::Quote(children) => visit(children, urls),
                _ => {}
            }
        }
    }
    let mut urls = Vec::new();
    visit(blocks, &mut urls);
    urls.into_iter().cloned().collect()
}

fn release_choices<'a>(
    releases: impl IntoIterator<Item = &'a ProjectRelease>,
) -> Vec<(String, String)> {
    releases
        .into_iter()
        .map(|item| {
            let v = &item.version;
            let mut parts = vec![channel(&v.version_type)];
            if let Some(date) = v
                .date_published
                .split('T')
                .next()
                .filter(|date| !date.is_empty())
            {
                parts.push(date.to_owned());
            }
            if let Some(bytes) = item.file_size {
                parts.push(size_label(bytes));
            }
            (
                v.id.clone(),
                format!("{} · {}", v.version_number, parts.join(" · ")),
            )
        })
        .collect()
}

impl Render for ProjectWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();
        let t = &palette;
        let body = match &self.page {
            Some(Ok(page)) => div()
                .flex()
                .flex_col()
                .gap(px(20.))
                .child(self.header(page, t))
                .child(
                    div().flex().flex_wrap().gap(px(6.)).children(
                        TABS.into_iter()
                            .enumerate()
                            .filter_map(|(index, (tab, label))| {
                                if tab == Tab::Gallery && page.gallery.is_empty() {
                                    return None;
                                }
                                Some(
                                    AuroraButton::new(("project-tab", index), label)
                                        .variant(ButtonVariant::Ghost)
                                        .selected(self.tab == tab)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.select_tab(tab, cx)
                                        })),
                                )
                            }),
                    ),
                )
                .child(ui::divider(t))
                .child(match self.tab {
                    Tab::Description if !self.description.is_empty() => {
                        self.markdown(&self.description, t, cx)
                    }
                    Tab::Description => ui::muted(t, "This project has no description."),
                    Tab::Gallery => self.gallery(page, t, cx),
                    Tab::Versions => self.versions(page, t, cx),
                    Tab::Changelog => self.changelog(page, t, cx),
                }),
            Some(Err(error)) => div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(12.))
                .child(ui::page_header(t, self.target.title.clone(), None))
                .child(ui::muted(t, error.message.clone()))
                .child(
                    AuroraButton::new("project-retry", "Retry").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.load_page();
                            cx.notify();
                        },
                    )),
                ),
            None => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(ui::page_header(t, self.target.title.clone(), None))
                .child(ui::muted(t, "Loading project details…")),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.background)
            .text_color(t.foreground)
            .font_family("Segoe UI")
            .when(chrome::custom(), |root| {
                root.child(chrome::title_bar(window, cx, t))
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .child(
                        div()
                            .id("project-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .p(px(24.))
                            .child(body),
                    )
                    .child(ui::scrollbar(t, "project-scrollbar", &self.scroll)),
            )
            .child(self.review.clone())
    }
}

impl ProjectWindow {
    fn header(&self, page: &ProjectPage, t: &AuroraTheme) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(16.))
                    .child(artwork::tile(
                        "project-icon",
                        self.icon.clone(),
                        &artwork::letter(&page.title),
                        px(artwork::CARD_ART_SIDE + 16.),
                        t,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(ui::TEXT_DISPLAY))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(page.title.clone()),
                            )
                            .children(
                                page.author
                                    .as_ref()
                                    .map(|author| ui::caption(t, format!("by {author}"))),
                            )
                            .child(ui::muted(t, page.summary.clone()))
                            .child(ui::caption(
                                t,
                                format!(
                                    "{} downloads · {} followers",
                                    grouped(page.downloads),
                                    grouped(page.followers)
                                ),
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(
                        page.loaders
                            .iter()
                            .map(|loader| ui::badge(t, loader_name(loader), ui::Tone::Accent)),
                    )
                    .children(
                        page.categories
                            .iter()
                            .map(|category| ui::badge(t, category.clone(), ui::Tone::Neutral)),
                    ),
            )
            .when(!page.links.is_empty(), |header| {
                header.child(div().flex().flex_wrap().gap(px(6.)).children(
                    page.links.iter().enumerate().map(|(index, link)| {
                        browser_link(("project-link", index), link.label, link.url.clone())
                    }),
                ))
            })
    }

    fn gallery(&self, page: &ProjectPage, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let Some(entry) = page.gallery.get(self.gallery_index) else {
            return div();
        };
        let count = page.gallery.len();
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(12.))
            .child(self.image(
                entry.title.as_deref().unwrap_or("Project screenshot"),
                Some(&entry.url),
                (0, 0),
                t,
                cx,
            ))
            .children(
                entry
                    .description
                    .as_ref()
                    .map(|text| ui::muted(t, text.clone())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("gallery-prev", "Previous")
                            .size(ButtonSize::Small)
                            .disabled(self.gallery_index == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.gallery_index = this.gallery_index.saturating_sub(1);
                                this.load_tab_artwork();
                                cx.notify();
                            })),
                    )
                    .child(ui::caption(
                        t,
                        format!("{} of {count}", self.gallery_index + 1),
                    ))
                    .child(
                        AuroraButton::new("gallery-next", "Next")
                            .size(ButtonSize::Small)
                            .disabled(self.gallery_index + 1 >= count)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.gallery_index = (this.gallery_index + 1).min(count - 1);
                                this.load_tab_artwork();
                                cx.notify();
                            })),
                    ),
            )
    }

    fn versions(&self, page: &ProjectPage, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        if let Some(error) = &page.releases_error {
            return self.releases_error(error, t, cx);
        }
        let target = self.target.target.as_ref();
        let matching = self.filters.matching(&page.releases);
        let selected = matching
            .iter()
            .find(|item| Some(&item.version.id) == self.filters.version.as_ref());
        let mut content = div().flex().flex_col().gap(px(12.));
        if let Some(target) = target {
            content = content.child(ui::caption(
                t,
                format!(
                    "Target instance · {} · Minecraft {} · {}",
                    target.display_name,
                    target.minecraft_version,
                    loader_name(&target.platform.kind),
                ),
            ));
        }
        if page.releases.is_empty() {
            return content.child(ui::muted(t, "No published versions."));
        }
        let minecraft = VersionFilters::minecraft_options(&page.releases);
        let loaders = self.filters.loader_options(&page.releases);
        content = content
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(12.))
                    .child(
                        self.picker(
                            Picker::Minecraft,
                            "Minecraft version",
                            minecraft
                                .into_iter()
                                .map(|value| (value.clone(), value))
                                .collect(),
                            self.filters.minecraft.as_deref(),
                            164.,
                            cx,
                        ),
                    )
                    .child(
                        self.picker(
                            Picker::Loader,
                            "Loader",
                            loaders
                                .into_iter()
                                .map(|loader| (loader.clone(), loader_name(&loader)))
                                .collect(),
                            self.filters.loader.as_deref(),
                            154.,
                            cx,
                        ),
                    ),
            )
            .when(!matching.is_empty(), |content| {
                content.child(self.picker(
                    Picker::Version,
                    "Project version",
                    release_choices(matching.iter().copied()),
                    self.filters.version.as_deref(),
                    350.,
                    cx,
                ))
            });
        let Some(release) = selected else {
            return content;
        };
        let v = &release.version;
        let published = v.date_published.split('T').next().unwrap_or_default();
        let file_size = release.file_size.map(size_label);
        let reason = v
            .unsupported_reason
            .as_deref()
            .or_else(|| install_block(self.target.kind, target, &self.filters));
        let busy = self
            .downloads
            .read(cx)
            .busy_projects()
            .contains(&page.project_id);
        let tone = match v.version_type.as_str() {
            "release" => ui::Tone::Success,
            "beta" => ui::Tone::Accent,
            "alpha" => ui::Tone::Warning,
            _ => ui::Tone::Neutral,
        };
        content.child(
            ui::card(t)
                .gap(px(10.))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(v.name.clone()),
                        )
                        .child(ui::badge(t, channel(&v.version_type), tone)),
                )
                .child(ui::caption(
                    t,
                    [Some(published), file_size.as_deref()]
                        .into_iter()
                        .flatten()
                        .filter(|text| !text.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · "),
                ))
                .when(!release.dependencies.is_empty(), |card| {
                    card.child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Dependencies"),
                    )
                    .children(release.dependencies.iter().map(|dependency| {
                        let name = dependency
                            .file_name
                            .as_deref()
                            .or(dependency.project_id.as_deref())
                            .or(dependency.version_id.as_deref())
                            .unwrap_or("Unspecified");
                        ui::caption(t, format!("{} · {}", name, dependency.kind))
                    }))
                })
                .children(reason.map(|reason| ui::muted(t, reason.to_owned())))
                .child(
                    div().flex().justify_end().child(
                        AuroraButton::new(
                            "project-install",
                            if busy { "In Downloads" } else { "Install" },
                        )
                        .icon("download")
                        .variant(ButtonVariant::Primary)
                        .disabled(reason.is_some() || busy)
                        .on_click(cx.listener(|this, _, window, cx| this.install(window, cx))),
                    ),
                ),
        )
    }

    fn changelog(&self, page: &ProjectPage, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        if let Some(error) = &page.releases_error {
            return self.releases_error(error, t, cx);
        }
        if page.releases.is_empty() {
            return ui::muted(t, "No published versions.");
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.picker(
                Picker::Changelog,
                "Project version",
                release_choices(&page.releases),
                self.changelog_version.as_deref(),
                350.,
                cx,
            ))
            .child(match &self.changelog {
                Some(Ok(notes)) if !notes.is_empty() => self.markdown(notes, t, cx),
                Some(Ok(_)) => ui::muted(t, "No changelog for this version."),
                Some(Err(error)) => ui::muted(t, error.clone()),
                None => ui::muted(t, "Loading changelog…"),
            })
    }

    fn releases_error(&self, error: &str, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(ui::muted(t, error.to_owned()))
            .child(
                AuroraButton::new("retry-releases", "Retry").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.load_page();
                        cx.notify();
                    },
                )),
            )
    }

    fn markdown(&self, blocks: &[markdown::Block], t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let mut index = 0;
        div()
            .max_w(px(740.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .text_size(px(ui::TEXT_BODY))
            .line_height(px(22.))
            .children(
                blocks
                    .iter()
                    .map(|block| self.block(block, t, cx, &mut index)),
            )
    }

    fn block(
        &self,
        block: &markdown::Block,
        t: &AuroraTheme,
        cx: &mut Context<Self>,
        index: &mut usize,
    ) -> AnyElement {
        let id = *index;
        *index += 1;
        match block {
            markdown::Block::Text(spans) => rich_text(id, spans, t),
            markdown::Block::Heading(level, spans) => div()
                .text_size(px(match level {
                    1 => ui::TEXT_TITLE,
                    2 => 20.,
                    _ => ui::TEXT_SECTION,
                }))
                .line_height(px(28.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(rich_text(id, spans, t))
                .into_any_element(),
            markdown::Block::Quote(blocks) => div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .border_l_2()
                .border_color(t.border)
                .pl(px(12.))
                .text_color(t.muted_foreground)
                .children(blocks.iter().map(|child| self.block(child, t, cx, index)))
                .into_any_element(),
            markdown::Block::List(start, items) => div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(items.iter().enumerate().map(|(i, child)| {
                    let marker = start.map_or_else(
                        || "•".into(),
                        |n| format!("{}.", n.saturating_add(i as u64)),
                    );
                    div()
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_none()
                                .text_color(t.muted_foreground)
                                .child(marker),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .child(self.block(child, t, cx, index)),
                        )
                }))
                .into_any_element(),
            markdown::Block::Item(blocks) => div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(blocks.iter().map(|child| self.block(child, t, cx, index)))
                .into_any_element(),
            markdown::Block::Code(code) | markdown::Block::Html(code) => div()
                .rounded(t.radius)
                .bg(t.sunken)
                .p(px(12.))
                .font_family("Consolas")
                .text_size(px(ui::TEXT_CAPTION))
                .child(code.trim_end_matches('\n').to_owned())
                .into_any_element(),
            markdown::Block::Image { alt, url } => self
                .image(alt, url.as_deref(), (1, id), t, cx)
                .into_any_element(),
            markdown::Block::Rule => ui::divider(t).into_any_element(),
        }
    }

    fn image(
        &self,
        alt: &str,
        url: Option<&str>,
        id: (usize, usize),
        t: &AuroraTheme,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut media = div().flex().flex_col().items_start().gap(px(6.));
        let alt = if alt.is_empty() { "Project image" } else { alt };
        if let Some(url) = url {
            let key = url.to_owned();
            if let Some(image) = self.artwork.image(&key) {
                let dimensions = image.size(0);
                let width = dimensions.width.0.min(740) as f32;
                let height = dimensions.height.0 as f32 * width / dimensions.width.0 as f32;
                return media
                    .child(
                        img(image)
                            .w(px(width))
                            .max_w_full()
                            .h(px(height))
                            .max_h(px(420.))
                            .object_fit(ObjectFit::Contain)
                            .rounded(t.radius),
                    )
                    .child(ui::caption(t, alt.to_owned()));
            }
            if self.pending.contains(&key) {
                return media.child(ui::muted(t, format!("Loading {alt}…")));
            }
            if url.starts_with("https://cdn.modrinth.com/data/") && !self.artwork.contains(&key) {
                return media.child(
                    AuroraButton::new(("load-project-image", id.1), format!("Load {alt}"))
                        .size(ButtonSize::Small)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.load_media(key.clone());
                            cx.notify();
                        })),
                );
            }
            media = media.child(browser_link(
                ("view-project-image", id.1),
                alt.to_owned(),
                key,
            ));
        } else {
            media = media.child(ui::caption(t, alt.to_owned()));
        }
        media
    }
}

fn browser_link(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    url: String,
) -> AuroraButton {
    AuroraButton::new(id, text)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
        .icon("open_in_new")
        .on_click(move |_, _, cx| {
            if let Some(url) = safe_web_link(&url, None) {
                cx.open_url(&url);
            }
        })
}

fn rich_text(id: usize, spans: &[markdown::Span], t: &AuroraTheme) -> AnyElement {
    let mut text = String::new();
    let mut styles = Vec::new();
    let mut fonts = Vec::new();
    let mut links = Vec::new();
    let mut ranges = Vec::new();
    for span in spans {
        let start = text.len();
        text.push_str(&span.text);
        let range = start..text.len();
        if range.is_empty() {
            continue;
        }
        if span.style.code {
            fonts.push((range.clone(), SharedString::from("Consolas")));
        }
        if let Some(url) = &span.link {
            ranges.push(range.clone());
            links.push(url.clone());
        }
        styles.push((
            range,
            HighlightStyle {
                color: span.link.as_ref().map(|_| t.ring),
                underline: span.link.as_ref().map(|_| UnderlineStyle {
                    thickness: px(1.),
                    ..Default::default()
                }),
                font_style: span.style.italic.then_some(FontStyle::Italic),
                font_weight: span.style.bold.then_some(FontWeight::BOLD),
                background_color: span.style.code.then_some(t.sunken),
                ..Default::default()
            },
        ));
    }
    let styled = StyledText::new(text)
        .with_highlights(styles)
        .with_font_family_overrides(fonts);
    if links.is_empty() {
        styled.into_any_element()
    } else {
        InteractiveText::new(("project-paragraph", id), styled)
            .on_click(ranges, move |index, _, cx| cx.open_url(&links[index]))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(id: &str, mc: &[&str], loaders: &[&str]) -> ProjectRelease {
        ProjectRelease {
            version: VersionChoice {
                id: id.into(),
                name: id.into(),
                version_number: id.into(),
                version_type: "release".into(),
                date_published: String::new(),
                environment: String::new(),
                loaders: loaders.iter().map(|v| (*v).into()).collect(),
                game_versions: mc.iter().map(|v| (*v).into()).collect(),
                archive_size: None,
                unsupported_reason: None,
            },
            file_size: None,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn filters_keep_only_published_combinations_and_reset_the_exact_pin() {
        let releases = [
            release("A", &["1.21.1"], &["fabric", "quilt"]),
            release("B", &["1.21.1", "1.21.2"], &["forge"]),
            release("C", &["1.21.2"], &["forge"]),
        ];
        let mut filters = VersionFilters::default();
        assert!(filters.matching(&releases).is_empty());
        filters.choose_minecraft("1.21.1".into(), &releases, 0, None);
        assert_eq!(
            filters.loader_options(&releases),
            ["fabric", "quilt", "forge"]
        );
        assert!(filters.matching(&releases).is_empty());
        filters.loader = Some("fabric".into());
        filters.version = Some("A".into());
        assert_eq!(filters.matching(&releases)[0].version.id, "A");
        filters.choose_minecraft("1.21.2".into(), &releases, 0, None);
        assert_eq!(filters.loader.as_deref(), Some("forge"));
        assert_eq!(filters.version, None);
        assert_eq!(
            filters
                .matching(&releases)
                .iter()
                .map(|r| r.version.id.as_str())
                .collect::<Vec<_>>(),
            ["B", "C"]
        );
    }

    #[test]
    fn target_defaults_to_its_loader_and_other_loader_choices_cannot_install() {
        let instance: api::InstanceSummary = serde_json::from_value(serde_json::json!({
            "id": "target", "displayName": "Fabric instance", "state": "ready", "aurora": null,
            "platform": {"kind": "fabric", "version": "0.16.0"}, "minecraftVersion": "1.21.1",
            "configuration": {"minecraftVersion": "1.21.1", "auroraEnabled": false,
                "loader": {"kind": "fabric", "version": "0.16.0"}, "memoryMib": 4096,
                "additionalJvmArguments": "", "window": null}
        }))
        .unwrap();
        let releases = [release("exact", &["1.21.1"], &["fabric", "quilt"])];
        let mut filters = VersionFilters::default();
        filters.choose_minecraft(
            instance.minecraft_version.clone(),
            &releases,
            0,
            Some(&instance),
        );
        assert_eq!(filters.loader.as_deref(), Some("fabric"));
        assert_eq!(filters.version, None);
        assert_eq!(install_block(0, Some(&instance), &filters), None);
        filters.loader = Some("quilt".into());
        assert_eq!(filters.matching(&releases)[0].version.id, "exact");
        assert!(install_block(0, Some(&instance), &filters).is_some());
    }
}
