//! Invented cosmetics for the marker-gated, debug-only showcase host.
//! No credentials, profile requests or skin-change service calls are used.
use crate::api;
use aurora_launcher_lib::host::Host;

pub fn state() -> String {
    std::env::var("AURORA_SHOWCASE_SKINS").unwrap_or_else(|_| "library".into())
}

pub fn accounts() -> api::AccountsState {
    let state = state();
    if state == "no-account" || state == "empty" {
        return api::AccountsState::default();
    }
    api::AccountsState {
        selected_account_id: Some("5e0c0a5e0c0a5e0c0a5e0c0a5e0c0a01".into()),
        accounts: vec![api::AccountSummary {
            account_id: "5e0c0a5e0c0a5e0c0a5e0c0a5e0c0a01".into(),
            minecraft_name: "Aurelia".into(),
            status: if state == "reauth" {
                "reauthenticationRequired"
            } else {
                "signedIn"
            }
            .into(),
        }],
    }
}

fn capes() -> Vec<api::Cape> {
    [
        ("migrator", "Migrator", [145, 55, 66, 255]),
        ("vanilla", "Vanilla", [83, 134, 103, 255]),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (id, name, color))| {
        let mut rgba = vec![0; 64 * 32 * 4];
        for y in 0..32 {
            for x in 0..64 {
                let offset = (y * 64 + x) * 4;
                let light = x % 10 < 2 || y % 16 < 2;
                rgba[offset..offset + 4].copy_from_slice(if light {
                    &[204, 183, 119, 255]
                } else {
                    &color
                });
            }
        }
        api::Cape {
            id: id.into(),
            name: name.into(),
            selected: index == 0,
            preview: Some(api::Texture {
                width: 64,
                height: 32,
                rgba,
            }),
        }
    })
    .collect()
}

pub async fn cosmetics(host: Host) -> api::ApiResult<api::Cosmetics> {
    let preset = if state() == "success" { 2 } else { 1 };
    let skin = api::skin_preview(host, format!("00000000-0000-0000-0000-{preset:012}")).await?;
    Ok(api::Cosmetics {
        current_skin_model: Some(skin.model.clone()),
        has_current_skin: true,
        current_skin: Some(skin),
        capes: capes(),
    })
}

pub async fn apply(host: Host, preset: String) -> api::ApiResult<api::Cosmetics> {
    let skin = api::skin_preview(host, preset).await?;
    Ok(api::Cosmetics {
        current_skin_model: Some(skin.model.clone()),
        has_current_skin: true,
        current_skin: Some(skin),
        capes: capes(),
    })
}
