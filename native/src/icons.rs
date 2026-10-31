//! Instance icons: the searchable Material Symbols picker and its two uses,
//! the identity dialog (name + icon, persisted through the core's icon
//! store) and instance creation (applied once the instance exists).

use slint::ComponentHandle;

use crate::symbols;
use crate::ui;
use crate::{AppWindow, IconPicker, Instances, SymbolCell};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    #[default]
    Identity,
    Create,
}

/// Picker state. `chosen == None` means the default icon.
#[derive(Default)]
pub struct PickerState {
    pub target: Target,
    pub chosen: Option<String>,
    /// The icon chosen for the instance being created.
    pub create_choice: Option<String>,
}

pub fn bind(window: &AppWindow) {
    let picker = window.global::<IconPicker>();
    let mut categories: Vec<slint::SharedString> = vec!["All categories".into()];
    categories.extend(
        symbols::CATEGORIES
            .iter()
            .map(|c| c.replace('&', " & ").into()),
    );
    picker.set_categories(ui::model(categories));
    picker.on_search(|_| render());
    picker.on_category_changed(|_| render());
    picker.on_choose(|name| {
        ui::with(|_, state| state.icon_picker.chosen = Some(name.to_string()));
        render_choice();
    });
    picker.on_reset(|| {
        ui::with(|_, state| state.icon_picker.chosen = None);
        render_choice();
    });
    picker.on_confirm(|| {
        ui::with(|window, state| {
            if state.icon_picker.target == Target::Create {
                state.icon_picker.create_choice = state.icon_picker.chosen.clone();
            }
            window.global::<IconPicker>().set_open(false);
        });
        render_create_choice();
    });
    picker.on_cancel(|| {
        ui::with(|window, _| window.global::<IconPicker>().set_open(false));
    });
    window.global::<Instances>().on_create_choose_icon(|| {
        let current = ui::state(|s| s.icon_picker.create_choice.clone());
        begin(Target::Create, current);
        ui::with(|window, _| window.global::<IconPicker>().set_open(true));
    });
    window.global::<Instances>().on_edit_icon(|| {
        ui::with(|window, _| window.global::<Instances>().invoke_rename_start());
    });
}

/// Resets the picker for a new use with `current` preselected.
pub fn begin(target: Target, current: Option<String>) {
    ui::with(|window, state| {
        state.icon_picker.target = target;
        state.icon_picker.chosen = current;
        let picker = window.global::<IconPicker>();
        picker.set_query("".into());
        picker.set_category(0);
    });
    render();
    render_choice();
}

/// The icon the picker currently holds.
pub fn chosen() -> Option<String> {
    ui::state(|s| s.icon_picker.chosen.clone())
}

/// A new creation dialog starts with the default icon.
pub fn reset_create_choice() {
    ui::with(|_, state| state.icon_picker.create_choice = None);
    render_create_choice();
}

pub fn take_create_choice() -> Option<String> {
    ui::with(|_, state| state.icon_picker.create_choice.take()).flatten()
}

fn render() {
    ui::with(|window, _| {
        let picker = window.global::<IconPicker>();
        let query = picker.get_query().to_string();
        let category = match picker.get_category() {
            index if index > 0 => Some(index as usize - 1),
            _ => None,
        };
        let results = symbols::search(&query, category, symbols::RESULT_LIMIT);
        let summary = match (results.len(), query.trim().is_empty()) {
            (0, _) => {
                "No icons match. Try another word, such as sword, server or tools.".to_owned()
            }
            (n, true) if n >= symbols::RESULT_LIMIT => {
                "Suggested icons. Search to find more.".to_owned()
            }
            (n, false) if n >= symbols::RESULT_LIMIT => {
                format!("First {n} matches. Add a word to narrow them.")
            }
            (1, _) => "1 icon".to_owned(),
            (n, _) => format!("{n} icons"),
        };
        picker.set_summary(summary.into());
        picker.set_results(ui::model(
            results
                .into_iter()
                .map(|symbol| SymbolCell {
                    name: symbol.name.into(),
                    label: symbols::label(symbol.name).into(),
                    path: symbol.path.into(),
                })
                .collect(),
        ));
    });
}

fn render_choice() {
    ui::with(|window, state| {
        let picker = window.global::<IconPicker>();
        let chosen = state.icon_picker.chosen.clone();
        let shown = chosen.as_deref().unwrap_or(symbols::FALLBACK_ICON);
        picker.set_chosen(chosen.clone().unwrap_or_default().into());
        picker.set_chosen_path(symbols::path_or_fallback(chosen.as_deref()).into());
        picker.set_chosen_label(symbols::label(shown).into());
    });
}

fn render_create_choice() {
    ui::with(|window, state| {
        let choice = state.icon_picker.create_choice.clone();
        let instances = window.global::<Instances>();
        instances.set_create_icon(symbols::path_or_fallback(choice.as_deref()).into());
        instances.set_create_icon_label(
            symbols::label(choice.as_deref().unwrap_or(symbols::FALLBACK_ICON)).into(),
        );
    });
}

/// Persists `icon` for `instance_id` (None clears it) and reloads state.
pub fn persist(instance_id: String, icon: Option<String>, then: impl FnOnce() + 'static) {
    let token = ui::park(then);
    ui::spawn(
        crate::api::set_instance_icon(ui::host(), instance_id, icon),
        move |result| {
            if let Err(error) = result {
                ui::error_toast(&error);
            }
            crate::instances::reload(move || ui::resume(token));
        },
    );
}
