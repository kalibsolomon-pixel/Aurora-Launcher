//! Accounts: Microsoft sign-in through the system browser (OAuth PKCE in
//! `auth::flow`), selection, session refresh and sign-out. The native UI never
//! sees a password or token; it renders only non-secret account summaries.

use slint::ComponentHandle;

use crate::api::{self, AccountsState};
use crate::ui;
use crate::{AccountRow, Accounts, AppWindow, Tone};

pub fn bind(window: &AppWindow) {
    let accounts = window.global::<Accounts>();
    accounts.on_open_dialog(open_dialog);
    accounts.on_sign_in(sign_in);
    accounts.on_cancel_sign_in(|| {
        ui::spawn(api::cancel_login(), |result| {
            if let Err(error) = result {
                ui::error_toast(&error);
            }
        });
    });
    accounts.on_select(|id| {
        ui::spawn(api::select_account(ui::host(), id.to_string()), |result| {
            if let Err(error) = result {
                ui::error_toast(&error);
            }
            reload();
        });
    });
    accounts.on_remove(|id| {
        ui::spawn(api::remove_account(ui::host(), id.to_string()), |result| {
            match result {
                Ok(()) => ui::toast(
                    "Signed out. The stored credential was removed.",
                    Tone::Success,
                ),
                Err(error) => ui::error_toast(&error),
            }
            reload();
        });
    });
    accounts.on_refresh(|id| {
        ui::spawn(api::refresh_session(ui::host(), id.to_string()), |result| {
            if let Err(error) = result {
                ui::error_toast(&error);
            }
            reload();
        });
    });
}

pub fn open_dialog() {
    ui::with(|window, _| {
        let accounts = window.global::<Accounts>();
        accounts.set_error("".into());
        accounts.set_dialog_open(true);
    });
}

pub fn reload() {
    ui::spawn(api::accounts(ui::host()), |result| {
        match result {
            Ok(state) => apply(state),
            Err(error) => {
                ui::with(|window, _| window.global::<Accounts>().set_error(error.message.into()));
            }
        }
        crate::home::refresh_readiness();
    });
}

pub fn apply(state: AccountsState) {
    ui::with(|_, app| app.accounts = state);
    render();
    let on_skins =
        ui::with(|window, _| window.global::<crate::Nav>().get_page() == crate::Page::Skins)
            .unwrap_or(false);
    if on_skins {
        crate::skins::load();
    }
    load_avatars();
}

/// Known cached heads render immediately; signed-in accounts without one
/// are fetched once (bounded, decoded natively; no URL reaches the UI).
fn load_avatars() {
    ui::spawn(api::cached_avatars(ui::host()), |result| {
        let cached = result.unwrap_or_default();
        let missing: Vec<String> = ui::with(|_, state| {
            for entry in cached {
                if let Some(image) = ui::rgba_image(&entry.avatar.rgba) {
                    state.avatars.insert(entry.account_id, image);
                }
            }
            state
                .accounts
                .accounts
                .iter()
                .filter(|a| a.status == "signedIn" && !state.avatars.contains_key(&a.account_id))
                .map(|a| a.account_id.clone())
                .collect()
        })
        .unwrap_or_default();
        render();
        for id in missing {
            let key = id.clone();
            ui::spawn(api::account_avatar(ui::host(), id), move |result| {
                if let Ok(Some(avatar)) = result {
                    if let Some(image) = ui::rgba_image(&avatar.rgba) {
                        ui::with(|_, state| state.avatars.insert(key, image));
                        render();
                    }
                }
            });
        }
    });
}

fn status_text(status: &str) -> &'static str {
    match status {
        "signedIn" => "Signed in",
        "reauthenticationRequired" => "Sign in again to play",
        _ => "Unavailable",
    }
}

pub fn render() {
    ui::with(|window, state| {
        let accounts = window.global::<Accounts>();
        let selected = state.accounts.selected_account_id.clone();
        let rows: Vec<AccountRow> = state
            .accounts
            .accounts
            .iter()
            .map(|account| {
                let avatar = state.avatars.get(&account.account_id).cloned();
                AccountRow {
                    id: account.account_id.clone().into(),
                    name: account.minecraft_name.clone().into(),
                    initial: ui::letter(&account.minecraft_name).into(),
                    status: status_text(&account.status).into(),
                    selected: selected.as_deref() == Some(account.account_id.as_str()),
                    has_avatar: avatar.is_some(),
                    avatar: avatar.unwrap_or_default(),
                }
            })
            .collect();
        let current = state
            .accounts
            .accounts
            .iter()
            .find(|a| selected.as_deref() == Some(a.account_id.as_str()));
        accounts.set_has_account(current.is_some());
        if let Some(current) = current {
            accounts.set_name(current.minecraft_name.clone().into());
            accounts.set_initial(ui::letter(&current.minecraft_name).into());
            accounts.set_status(status_text(&current.status).into());
            let avatar = state.avatars.get(&current.account_id).cloned();
            accounts.set_has_avatar(avatar.is_some());
            accounts.set_avatar(avatar.unwrap_or_default());
        }
        accounts.set_rows(ui::model(rows));
    });
}

fn sign_in() {
    ui::with(|window, _| {
        let accounts = window.global::<Accounts>();
        accounts.set_login_busy(true);
        accounts.set_error("".into());
        accounts.set_login_phase(
            "Continue in your browser. Sign in with Microsoft, then return here.".into(),
        );
    });
    ui::spawn(api::begin_login(ui::host()), |result| {
        ui::with(|window, _| window.global::<Accounts>().set_login_busy(false));
        match result {
            Ok(account) => {
                ui::toast(
                    format!("Signed in as {}.", account.minecraft_name),
                    Tone::Success,
                );
                ui::with(|window, _| window.global::<Accounts>().set_dialog_open(false));
            }
            Err(error)
                if error.code == "auth_cancelled" || error.code == "auth_login_cancelled" => {}
            Err(error) => {
                ui::with(|window, _| window.global::<Accounts>().set_error(error.message.into()));
            }
        }
        reload();
    });
}

/// `auth-progress` event: coarse phases only, never credentials.
pub fn on_auth_progress(phase: &str) {
    let text = match phase {
        "waitingForMicrosoft" => {
            "Continue in your browser. Sign in with Microsoft, then return here."
        }
        "exchangingMicrosoftToken" => "Confirming your Microsoft sign-in…",
        "authenticatingWithXbox" => "Signing in to Xbox services…",
        "authorizingXsts" => "Authorizing with Xbox services…",
        "authenticatingMinecraft" => "Signing in to Minecraft…",
        "checkingEntitlement" => "Checking that you own Minecraft…",
        "fetchingProfile" => "Loading your Minecraft profile…",
        "savingAccount" => "Saving your account…",
        "restoringSession" => "Restoring your session…",
        _ => "Signing in…",
    };
    ui::with(|window, _| window.global::<Accounts>().set_login_phase(text.into()));
}
