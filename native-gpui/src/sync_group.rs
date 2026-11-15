//! The synced group card on the Sync page: choose a main instance and members,
//! save, sync now, and restore. The backend owns every file decision; this
//! view only drafts the group and shows what happened.
use super::*;

#[derive(Default)]
pub(super) struct GroupView {
    overview: Option<api::SyncGroupOverview>,
    draft: Option<api::SyncGroupConfig>,
    serial: u64,
    busy: bool,
    message: String,
}

impl GroupView {
    fn dirty(&self) -> bool {
        matches!((&self.overview, &self.draft), (Some(o), Some(d)) if o.config != *d)
    }

    /// The saved group, not the draft, decides whether it is in charge.
    pub(super) fn active(&self) -> bool {
        self.overview
            .as_ref()
            .is_some_and(|o| o.config.enabled && o.config.main_instance_id.is_some())
    }

    /// A first draft starts from the legacy pool's members and suggests the
    /// most recently played of them as the main.
    fn draft_from(overview: &api::SyncGroupOverview) -> api::SyncGroupConfig {
        let mut draft = overview.config.clone();
        if draft.main_instance_id.is_none() {
            draft.members = overview
                .instances
                .iter()
                .filter(|i| i.member)
                .map(|i| i.id.clone())
                .collect();
            draft.main_instance_id = overview
                .suggested_main
                .clone()
                .or_else(|| draft.members.first().cloned());
        }
        draft
    }
}

fn relative(at: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let minutes = now.saturating_sub(at) / 60_000;
    match minutes {
        0 => "just now".into(),
        1..=59 => format!("{minutes} min ago"),
        60..=1439 => format!("{} h ago", minutes / 60),
        _ => format!("{} d ago", minutes / 1440),
    }
}

fn plural(count: usize, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

impl Syncing {
    pub(super) fn read_group(&mut self) {
        let Some(host) = self.host.clone() else {
            return;
        };
        self.group.serial = self.group.serial.wrapping_add(1);
        let serial = self.group.serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::sync_group(host)).await;
            let _ = tx.send(Reply::Group(serial, r)).await;
        });
    }

    pub(super) fn receive_group(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Group(serial, result) if serial == self.group.serial => match result {
                Ok(overview) => {
                    if !self.group.dirty() && !self.group.busy {
                        self.group.draft = Some(GroupView::draft_from(&overview));
                    }
                    self.group.overview = Some(overview);
                }
                Err(e) => self.group.message = error(e),
            },
            Reply::GroupSaved(result) => {
                self.group.busy = false;
                match result {
                    Ok(overview) => {
                        self.group.draft = Some(GroupView::draft_from(&overview));
                        self.group.overview = Some(overview);
                        self.group.message = "Saved.".into();
                        // Turning the group on switches the legacy pool off.
                        self.refresh(cx);
                        cx.emit(SyncingEvent::Changed);
                    }
                    Err(e) => self.group.message = error(e),
                }
            }
            Reply::GroupSynced(result) => {
                self.group.busy = false;
                self.group.message = match result {
                    Ok(report) => report.message,
                    Err(e) => error(e),
                };
                self.read_group();
            }
            _ => {}
        }
    }

    fn edit_group(&mut self, edit: impl FnOnce(&mut api::SyncGroupConfig), cx: &mut Context<Self>) {
        if self.group.busy {
            return;
        }
        if let Some(draft) = self.group.draft.as_mut() {
            edit(draft);
            self.group.message.clear();
            cx.notify();
        }
    }

    fn save_group(&mut self, cx: &mut Context<Self>) {
        let (Some(host), Some(draft)) = (self.host.clone(), self.group.draft.clone()) else {
            return;
        };
        if self.group.busy {
            return;
        }
        self.group.busy = true;
        self.group.message = "Saving…".into();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::sync_group_save(host, draft)).await;
            let _ = tx.send(Reply::GroupSaved(r)).await;
        });
        cx.notify();
    }

    fn sync_group_now(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.group.busy {
            return;
        }
        self.group.busy = true;
        self.group.message = "Syncing stopped members…".into();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::sync_group_now(host)).await;
            let _ = tx.send(Reply::GroupSynced(r)).await;
        });
        cx.notify();
    }

    fn restore_group(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.group.busy {
            return;
        }
        self.group.busy = true;
        self.group.message = "Restoring…".into();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::sync_group_restore(host, id)).await;
            let _ = tx.send(Reply::GroupSaved(r)).await;
        });
        cx.notify();
    }

    pub(super) fn render_group(
        &mut self,
        t: &crate::theme::AuroraTheme,
        cx: &mut Context<Self>,
    ) -> Div {
        let view = cx.entity();
        let mut card = ui::card(t).gap(px(14.)).child(ui::section_header(
            t,
            "Synced group",
            Some(
                "Choose a main instance. Instances take its settings when they join the group; after that, a change made in any member reaches the others before they next launch."
                    .into(),
            ),
        ));
        let (Some(overview), Some(draft)) = (self.group.overview.clone(), self.group.draft.clone())
        else {
            return card.child(ui::caption(t, "Loading the synced group…"));
        };
        let busy = self.group.busy;
        let dirty = self.group.dirty();
        let (badge, tone) = if dirty {
            ("Unsaved changes", ui::Tone::Warning)
        } else if self.group.active() {
            ("On", ui::Tone::Success)
        } else {
            ("Off", ui::Tone::Neutral)
        };
        card = card.child(
            ui::setting_row(
                t,
                "Use a synced group",
                Some(
                    if draft.enabled {
                        "Settings follow the group. File sync below is turned off while the group is on."
                    } else {
                        "Off. Each instance keeps its own settings."
                    }
                    .into(),
                ),
                false,
            )
            .py(px(0.))
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(16.))
                    .child(ui::status_badge(t, badge, tone))
                    .child(
                        AuroraToggle::new("sync-group-enabled")
                            .label("Use a synced group")
                            .checked(draft.enabled)
                            .disabled(busy)
                            .on_change({
                                let view = view.clone();
                                move |next: bool, _, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.edit_group(|d| d.enabled = next, cx)
                                    });
                                }
                            }),
                    ),
            ),
        );

        // Instances: membership, main choice and live state.
        let mut list = div().flex().flex_col().gap(px(6.));
        for instance in &overview.instances {
            let id = instance.id.clone();
            let main = draft.main_instance_id.as_deref() == Some(id.as_str());
            let member = main || draft.members.contains(&id);
            let mut badges = div().flex().items_center().gap(px(8.));
            if main {
                badges = badges.child(ui::status_badge(t, "Main", ui::Tone::Accent));
            } else if member && (instance.pending_seed || !overview.config.members.contains(&id)) {
                badges = badges.child(ui::status_badge(
                    t,
                    "Takes main's settings next launch",
                    ui::Tone::Warning,
                ));
            }
            if instance.running {
                badges = badges.child(ui::status_badge(t, "Running", ui::Tone::Neutral));
            }
            let row =
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .py(px(4.))
                    .child(
                        AuroraCheckbox::new(
                            SharedString::from(format!("sync-group-member-{id}")),
                            format!("{} · {}", instance.name, instance.minecraft_version),
                        )
                        .checked(member)
                        .disabled(busy || main)
                        .on_change({
                            let view = view.clone();
                            let id = id.clone();
                            move |next: bool, _, _, cx| {
                                let id = id.clone();
                                view.update(cx, |this, cx| {
                                    this.edit_group(
                                        |d| {
                                            d.members.retain(|m| *m != id);
                                            if next {
                                                d.members.push(id);
                                            }
                                        },
                                        cx,
                                    )
                                });
                            }
                        }),
                    )
                    .child(div().flex().items_center().gap(px(10.)).child(badges).when(
                        !main,
                        |row| {
                            row.child(
                                AuroraButton::new(
                                    SharedString::from(format!("sync-group-main-{id}")),
                                    "Make main",
                                )
                                .size(ButtonSize::Small)
                                .variant(ButtonVariant::Ghost)
                                .disabled(busy)
                                .on_click({
                                    let view = view.clone();
                                    let id = id.clone();
                                    move |_, _, cx| {
                                        let id = id.clone();
                                        view.update(cx, |this, cx| {
                                            this.edit_group(
                                                |d| {
                                                    if !d.members.contains(&id) {
                                                        d.members.push(id.clone());
                                                    }
                                                    d.main_instance_id = Some(id);
                                                },
                                                cx,
                                            )
                                        });
                                    }
                                }),
                            )
                        },
                    ));
            list = list.child(row);
        }
        card = card.child(list);

        // What follows the group.
        let choice = |id: &'static str,
                      label: &'static str,
                      checked: bool,
                      edit: fn(&mut api::SyncGroupConfig, bool)| {
            let view = view.clone();
            AuroraCheckbox::new(id, label)
                .checked(checked)
                .disabled(busy)
                .on_change(move |next: bool, _, _, cx| {
                    view.update(cx, |this, cx| this.edit_group(|d| edit(d, next), cx));
                })
        };
        card = card
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.))
                    .child(choice("sync-group-vanilla", "Game settings, controls and graphics", draft.vanilla, |d, v| d.vanilla = v))
                    .child(choice("sync-group-aurora", "Aurora settings and profiles", draft.aurora, |d, v| d.aurora = v))
                    .child(choice("sync-group-packs", "Resource and shader packs", draft.packs, |d, v| d.packs = v))
                    .child(choice("sync-group-join", "New instances join the group", draft.new_instances_join, |d, v| d.new_instances_join = v)),
            )
            .child(ui::caption(
                t,
                "Never synced: worlds, waypoints and captured maps, playtime and fight counters, sound device, and consent choices. Files are backed up before every change, and nothing is written while that instance's game is running.",
            ));

        // Actions.
        let can_sync = self.group.active() && !dirty && !busy;
        card = card.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(10.))
                .child(
                    AuroraButton::new("sync-group-save", "Save group")
                        .variant(ButtonVariant::Primary)
                        .disabled(busy || !dirty)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| view.update(cx, |this, cx| this.save_group(cx))
                        }),
                )
                .when(dirty, |row| {
                    row.child(
                        AuroraButton::new("sync-group-revert", "Revert")
                            .disabled(busy)
                            .on_click({
                                let view = view.clone();
                                move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        if let Some(o) = &this.group.overview {
                                            this.group.draft = Some(GroupView::draft_from(o));
                                        }
                                        cx.notify();
                                    })
                                }
                            }),
                    )
                })
                .child(
                    AuroraButton::new("sync-group-now", "Sync now")
                        .icon("sync")
                        .disabled(!can_sync)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| view.update(cx, |this, cx| this.sync_group_now(cx))
                        }),
                )
                .when(!self.group.message.is_empty(), |row| {
                    row.child(ui::caption(t, self.group.message.clone()))
                }),
        );

        // Recent changes with restore.
        if !overview.history.is_empty() {
            let name = |id: &str| {
                overview
                    .instances
                    .iter()
                    .find(|i| i.id == id)
                    .map_or_else(|| "Removed instance".to_owned(), |i| i.name.clone())
            };
            let mut history = div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(ui::section_header(t, "Recent changes", None));
            for entry in overview.history.iter().take(8) {
                let verb = if entry.kind == "seed" {
                    "Gave main's settings to"
                } else {
                    "Updated"
                };
                let mut detail = format!(
                    "{verb} {} · {}",
                    name(&entry.instance_id),
                    plural(entry.keys.len(), "setting")
                );
                if !entry.packs.is_empty() {
                    detail.push_str(&format!(" · {}", plural(entry.packs.len(), "pack")));
                }
                detail.push_str(&format!(" · {}", relative(entry.at)));
                let id = entry.id.clone();
                history = history.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .child(
                            div()
                                .min_w(px(0.))
                                .text_size(px(ui::TEXT_CAPTION))
                                .child(detail),
                        )
                        .when(!entry.files.is_empty(), |row| {
                            row.child(
                                AuroraButton::new(
                                    SharedString::from(format!("sync-group-restore-{id}")),
                                    "Restore and leave group",
                                )
                                .size(ButtonSize::Small)
                                .variant(ButtonVariant::Ghost)
                                .disabled(busy)
                                .on_click({
                                    let view = view.clone();
                                    move |_, _, cx| {
                                        let id = id.clone();
                                        view.update(cx, |this, cx| this.restore_group(id, cx))
                                    }
                                }),
                            )
                        }),
                );
            }
            card = card.child(history);
        }
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overview(config: api::SyncGroupConfig, suggested: Option<&str>) -> api::SyncGroupOverview {
        let member = |id: &str, member: bool| api::SyncGroupMember {
            id: id.into(),
            name: id.into(),
            minecraft_version: "26.3".into(),
            member,
            main: false,
            pending_seed: false,
            running: false,
        };
        api::SyncGroupOverview {
            config,
            instances: vec![member("a", true), member("b", false), member("c", true)],
            history: vec![],
            suggested_main: suggested.map(Into::into),
        }
    }

    #[test]
    fn a_first_draft_uses_current_sync_members_and_the_suggested_main() {
        let draft = GroupView::draft_from(&overview(api::SyncGroupConfig::default(), Some("c")));
        assert_eq!(draft.members, ["a", "c"]);
        assert_eq!(draft.main_instance_id.as_deref(), Some("c"));
        assert!(!draft.enabled, "the group is never switched on by loading");
    }

    #[test]
    fn a_saved_group_is_drafted_unchanged_and_edits_are_dirty() {
        let saved = api::SyncGroupConfig {
            enabled: true,
            main_instance_id: Some("a".into()),
            members: vec!["a".into(), "b".into()],
            ..api::SyncGroupConfig::default()
        };
        let o = overview(saved.clone(), Some("c"));
        let mut view = GroupView {
            draft: Some(GroupView::draft_from(&o)),
            overview: Some(o),
            ..GroupView::default()
        };
        assert_eq!(view.draft.as_ref(), Some(&saved));
        assert!(view.active() && !view.dirty());
        view.draft.as_mut().unwrap().packs = !saved.packs;
        assert!(view.dirty());
    }
}
