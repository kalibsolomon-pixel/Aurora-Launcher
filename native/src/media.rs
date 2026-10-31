//! The Media widget's system integration: Windows Global System Media
//! Transport Controls (the sessions behind the taskbar media flyout).
//!
//! - Event driven: a dedicated worker subscribes to session, media-property
//!   and playback changes and refreshes only when Windows reports one. There
//!   is no polling.
//! - Started only while the Media widget is on Home; removing the widget
//!   unregisters every handler and ends the worker.
//! - Only apps that publish a system media session appear (Spotify,
//!   browsers playing YouTube/Twitch, and similar). Nothing is injected into
//!   browsers and no browser data is read.
//! - Thumbnails come from the session, are capped at 2 MiB, and are decoded
//!   by the Windows imaging component into at most 96x96 RGBA.
//! - Volume is not offered: sessions expose no per-app volume, and the
//!   launcher never changes the system master volume.

use std::sync::Mutex;
use std::sync::mpsc::Sender;

use slint::ComponentHandle;

use crate::{Widgets, ui};

/// What the worker last read from Windows.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub source: String,
    pub playing: bool,
    pub can_toggle: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub sessions: usize,
    pub art: Option<(Vec<u8>, u32, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Refresh,
    Toggle,
    Next,
    Previous,
    Cycle,
    Stop,
}

static WORKER: Mutex<Option<Sender<Command>>> = Mutex::new(None);

pub fn supported() -> bool {
    cfg!(windows)
}

pub fn bind(window: &crate::AppWindow) {
    let widgets = window.global::<Widgets>();
    widgets.set_media_supported(supported());
    widgets.on_media_toggle(|| send(Command::Toggle));
    widgets.on_media_next(|| send(Command::Next));
    widgets.on_media_previous(|| send(Command::Previous));
    widgets.on_media_cycle(|| send(Command::Cycle));
}

fn send(command: Command) {
    if let Some(sender) = WORKER.lock().expect("media worker slot").as_ref() {
        let _ = sender.send(command);
    }
}

/// Starts or stops the worker to match whether the Media widget is shown.
pub fn set_enabled(enabled: bool) {
    let mut slot = WORKER.lock().expect("media worker slot");
    match (enabled && supported(), slot.is_some()) {
        (true, false) => *slot = imp::start(),
        (false, true) => {
            if let Some(sender) = slot.take() {
                let _ = sender.send(Command::Stop);
            }
            apply(Snapshot::default());
        }
        _ => {}
    }
}

fn apply(snapshot: Snapshot) {
    ui::with(|window, _| {
        let w = window.global::<Widgets>();
        w.set_media_active(snapshot.active);
        w.set_media_title(snapshot.title.into());
        w.set_media_artist(snapshot.artist.into());
        w.set_media_source(snapshot.source.into());
        w.set_media_playing(snapshot.playing);
        w.set_media_can_toggle(snapshot.can_toggle);
        w.set_media_can_next(snapshot.can_next);
        w.set_media_can_previous(snapshot.can_previous);
        w.set_media_session_count(snapshot.sessions as i32);
        match snapshot
            .art
            .and_then(|(rgba, width, height)| ui::rectangular_image(&rgba, width, height))
        {
            Some(image) => {
                w.set_media_art(image);
                w.set_media_has_art(true);
            }
            None => w.set_media_has_art(false),
        }
    });
}

/// "Spotify.exe" / "Chrome" / "308046B0AF4A39CB" → a short source label.
fn source_label(app_id: &str) -> String {
    let base = app_id.rsplit(['\\', '!']).next().unwrap_or(app_id);
    let base = base.strip_suffix(".exe").unwrap_or(base);
    let known = [
        ("spotify", "Spotify"),
        ("chrome", "Google Chrome"),
        ("msedge", "Microsoft Edge"),
        ("firefox", "Firefox"),
        ("opera", "Opera"),
        ("brave", "Brave"),
        ("vlc", "VLC"),
        ("zunemusic", "Media Player"),
    ];
    let lower = app_id.to_ascii_lowercase();
    for (needle, label) in known {
        if lower.contains(needle) {
            return label.into();
        }
    }
    if base.len() > 32 || base.chars().all(|c| c.is_ascii_hexdigit()) {
        "Media app".into()
    } else {
        base.chars().filter(|c| !c.is_control()).collect()
    }
}

#[cfg(windows)]
mod imp {
    use std::sync::mpsc::{self, Receiver, Sender};

    use windows::Foundation::TypedEventHandler;
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat,
        BitmapTransform, ColorManagementMode, ExifOrientationMode,
    };
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    use super::{Command, Snapshot, apply, source_label};

    const MAX_ART_BYTES: u64 = 2 * 1024 * 1024;
    const ART_SIZE: u32 = 96;

    pub fn start() -> Option<Sender<Command>> {
        let (sender, receiver) = mpsc::channel();
        let events = sender.clone();
        std::thread::Builder::new()
            .name("aurora-media".into())
            .spawn(move || run(events, receiver))
            .ok()?;
        let _ = sender.send(Command::Refresh);
        Some(sender)
    }

    struct Subscription {
        session: Session,
        properties: i64,
        playback: i64,
    }

    fn run(events: Sender<Command>, commands: Receiver<Command>) {
        use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let Ok(manager) = Manager::RequestAsync().and_then(|op| op.get()) else {
            eprintln!("[aurora-native] media sessions are unavailable on this system");
            return;
        };
        let notify = events.clone();
        let sessions_token = manager
            .SessionsChanged(&TypedEventHandler::new(move |_, _| {
                let _ = notify.send(Command::Refresh);
                Ok(())
            }))
            .ok();
        let notify = events.clone();
        let current_token = manager
            .CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
                let _ = notify.send(Command::Refresh);
                Ok(())
            }))
            .ok();
        let mut chosen: Option<String> = None;
        let mut subscription: Option<Subscription> = None;
        while let Ok(mut command) = commands.recv() {
            // Coalesce bursts of change notifications into one read.
            while command == Command::Refresh {
                match commands.try_recv() {
                    Ok(next) => command = next,
                    Err(_) => break,
                }
            }
            if command == Command::Stop {
                break;
            }
            let sessions = all_sessions(&manager);
            if command == Command::Cycle && sessions.len() > 1 {
                let ids: Vec<String> = sessions.iter().map(app_id).collect();
                let current = chosen
                    .as_ref()
                    .and_then(|c| ids.iter().position(|id| id == c))
                    .unwrap_or(0);
                chosen = Some(ids[(current + 1) % ids.len()].clone());
            }
            let session = chosen
                .as_ref()
                .and_then(|c| sessions.iter().find(|s| &app_id(s) == c).cloned())
                .or_else(|| manager.GetCurrentSession().ok())
                .or_else(|| sessions.first().cloned());
            if let Some(session) = &session {
                match command {
                    Command::Toggle => {
                        let _ = session.TryTogglePlayPauseAsync().and_then(|op| op.get());
                    }
                    Command::Next => {
                        let _ = session.TrySkipNextAsync().and_then(|op| op.get());
                    }
                    Command::Previous => {
                        let _ = session.TrySkipPreviousAsync().and_then(|op| op.get());
                    }
                    _ => {}
                }
            }
            // Follow change events for the session being shown only.
            let same = match (&subscription, &session) {
                (Some(current), Some(session)) => app_id(&current.session) == app_id(session),
                (None, None) => true,
                _ => false,
            };
            if !same {
                unsubscribe(subscription.take());
                subscription = session.as_ref().and_then(|s| subscribe(s, &events));
            }
            let snapshot = session
                .as_ref()
                .map(|s| read(s, sessions.len()))
                .unwrap_or_default();
            let _ = slint::invoke_from_event_loop(move || apply(snapshot));
        }
        unsubscribe(subscription.take());
        if let Some(token) = sessions_token {
            let _ = manager.RemoveSessionsChanged(token);
        }
        if let Some(token) = current_token {
            let _ = manager.RemoveCurrentSessionChanged(token);
        }
    }

    fn all_sessions(manager: &Manager) -> Vec<Session> {
        manager
            .GetSessions()
            .map(|list| list.into_iter().collect())
            .unwrap_or_default()
    }

    fn app_id(session: &Session) -> String {
        session
            .SourceAppUserModelId()
            .map(|id| id.to_string())
            .unwrap_or_default()
    }

    fn subscribe(session: &Session, events: &Sender<Command>) -> Option<Subscription> {
        let notify = events.clone();
        let properties = session
            .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                let _ = notify.send(Command::Refresh);
                Ok(())
            }))
            .ok()?;
        let notify = events.clone();
        let playback = session
            .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                let _ = notify.send(Command::Refresh);
                Ok(())
            }))
            .ok()?;
        Some(Subscription {
            session: session.clone(),
            properties,
            playback,
        })
    }

    fn unsubscribe(subscription: Option<Subscription>) {
        if let Some(s) = subscription {
            let _ = s.session.RemoveMediaPropertiesChanged(s.properties);
            let _ = s.session.RemovePlaybackInfoChanged(s.playback);
        }
    }

    fn bounded(text: windows::core::HSTRING) -> String {
        text.to_string()
            .chars()
            .filter(|c| !c.is_control())
            .take(160)
            .collect()
    }

    fn read(session: &Session, sessions: usize) -> Snapshot {
        let mut snapshot = Snapshot {
            active: true,
            sessions,
            source: source_label(&app_id(session)),
            ..Snapshot::default()
        };
        if let Ok(info) = session.GetPlaybackInfo() {
            snapshot.playing = info.PlaybackStatus().is_ok_and(|s| s == Status::Playing);
            if let Ok(controls) = info.Controls() {
                snapshot.can_toggle = controls.IsPlayPauseToggleEnabled().unwrap_or(false);
                snapshot.can_next = controls.IsNextEnabled().unwrap_or(false);
                snapshot.can_previous = controls.IsPreviousEnabled().unwrap_or(false);
            }
        }
        if let Ok(properties) = session.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
            snapshot.title = properties.Title().map(bounded).unwrap_or_default();
            snapshot.artist = properties.Artist().map(bounded).unwrap_or_default();
            snapshot.art = properties.Thumbnail().ok().and_then(|reference| {
                let stream = reference.OpenReadAsync().and_then(|op| op.get()).ok()?;
                if stream.Size().ok()? > MAX_ART_BYTES {
                    return None;
                }
                decode(&stream).ok()
            });
        }
        if snapshot.title.is_empty() {
            snapshot.title = "Playing".into();
        }
        snapshot
    }

    fn decode(
        stream: &windows::Storage::Streams::IRandomAccessStreamWithContentType,
    ) -> windows::core::Result<(Vec<u8>, u32, u32)> {
        let decoder = BitmapDecoder::CreateAsync(stream)?.get()?;
        let (width, height) = (decoder.PixelWidth()?, decoder.PixelHeight()?);
        if width == 0 || height == 0 {
            return Err(windows::core::Error::empty());
        }
        let scale = (ART_SIZE as f64 / width.max(height) as f64).min(1.0);
        let (out_w, out_h) = (
            ((width as f64 * scale).round() as u32).max(1),
            ((height as f64 * scale).round() as u32).max(1),
        );
        let transform = BitmapTransform::new()?;
        transform.SetScaledWidth(out_w)?;
        transform.SetScaledHeight(out_h)?;
        transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
        let pixels = decoder
            .GetPixelDataTransformedAsync(
                BitmapPixelFormat::Rgba8,
                BitmapAlphaMode::Straight,
                &transform,
                ExifOrientationMode::RespectExifOrientation,
                ColorManagementMode::DoNotColorManage,
            )?
            .get()?
            .DetachPixelData()?;
        if pixels.len() != (out_w * out_h * 4) as usize {
            return Err(windows::core::Error::empty());
        }
        Ok((pixels.to_vec(), out_w, out_h))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn start() -> Option<std::sync::mpsc::Sender<super::Command>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::source_label;

    #[test]
    fn source_labels_are_short_and_friendly() {
        assert_eq!(source_label("Spotify.exe"), "Spotify");
        assert_eq!(source_label("Chrome"), "Google Chrome");
        assert_eq!(source_label("MSEdge"), "Microsoft Edge");
        assert_eq!(source_label("308046B0AF4A39CB"), "Media app");
        assert_eq!(source_label("Foo.exe"), "Foo");
    }
}
