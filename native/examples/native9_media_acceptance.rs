//! Owns two silent synthetic media players. Never prints or controls a session
//! without its exact test AppUserModelID. No owner media metadata is retained.
#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use windows::{
        Foundation::TypedEventHandler,
        Media::Core::MediaSource,
        Media::Playback::MediaPlayer,
        Media::{
            MediaPlaybackStatus as Status, MediaPlaybackType,
            SystemMediaTransportControlsButton as Button,
        },
        Storage::StorageFile,
        core::HSTRING,
    };
    unsafe {
        windows::Win32::System::WinRT::RoInitialize(
            windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
        )?;
    }
    if let Some(id) = std::env::args().nth(1) {
        assert!(id.starts_with("Aurora.Native9.Synthetic."));
        unsafe {
            windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(
                &id,
            ))?;
        }
        let root =
            std::env::temp_dir().join(format!("aurora-native9-media-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("silence.wav");
        let frames = 16000u32 * 60;
        let length = frames * 2;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((length + 36).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(16000u32.to_le_bytes());
        wav.extend(32000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(length.to_le_bytes());
        wav.resize(length as usize + 44, 0);
        std::fs::write(&path, wav).unwrap();
        let player = MediaPlayer::new()?;
        player.SetVolume(0.0)?;
        player.SetIsLoopingEnabled(true)?;
        player.CommandManager()?.SetIsEnabled(false)?;
        player.SetSource(&MediaSource::CreateFromStorageFile(
            &StorageFile::GetFileFromPathAsync(&HSTRING::from(path.to_string_lossy().as_ref()))?
                .get()?,
        )?)?;
        let controls = player.SystemMediaTransportControls()?;
        controls.SetIsEnabled(true)?;
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsNextEnabled(!id.ends_with(".two"))?;
        controls.SetIsPreviousEnabled(!id.ends_with(".two"))?;
        let update = controls.DisplayUpdater()?;
        update.SetType(MediaPlaybackType::Music)?;
        update
            .MusicProperties()?
            .SetTitle(&HSTRING::from("Aurora synthetic track"))?;
        update
            .MusicProperties()?
            .SetArtist(&HSTRING::from("Disposable fixture"))?;
        let art = root.join("synthetic-art.png");
        std::fs::write(&art, include_bytes!("../assets/icon-128.png")).unwrap();
        update.SetThumbnail(
            &windows::Storage::Streams::RandomAccessStreamReference::CreateFromFile(
                &StorageFile::GetFileFromPathAsync(&HSTRING::from(art.to_string_lossy().as_ref()))?
                    .get()?,
            )?,
        )?;
        update.Update()?;
        let events = Arc::new(AtomicUsize::new(0));
        let seen = events.clone();
        let target = controls.clone();
        let token = controls.ButtonPressed(&TypedEventHandler::<
            windows::Media::SystemMediaTransportControls,
            windows::Media::SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |_, args| {
            if let Some(args) = args.as_ref() {
                match args.Button()? {
                    Button::Play => target.SetPlaybackStatus(Status::Playing)?,
                    Button::Pause => target.SetPlaybackStatus(Status::Paused)?,
                    Button::Next | Button::Previous => {
                        let update = target.DisplayUpdater()?;
                        update.MusicProperties()?.SetTitle(&HSTRING::from(
                            if args.Button()? == Button::Next {
                                "Aurora synthetic next"
                            } else {
                                "Aurora synthetic track"
                            },
                        ))?;
                        update.Update()?;
                    }
                    _ => return Ok(()),
                };
                seen.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }))?;
        player.Play()?;
        controls.SetPlaybackStatus(Status::Playing)?;
        println!("synthetic-ready");
        for _ in 0..300 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        controls.RemoveButtonPressed(token)?;
        controls.SetIsEnabled(false)?;
        player.Close()?;
        println!("synthetic-events={}", events.load(Ordering::SeqCst));
        return Ok(());
    }
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Manager;
    let exe = std::env::current_exe().unwrap();
    let prefix = format!("Aurora.Native9.Synthetic.{}", std::process::id());
    let mut children = Vec::new();
    for suffix in ["one", "two"] {
        children.push(
            std::process::Command::new(&exe)
                .arg(format!("{prefix}.{suffix}"))
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    struct Children(Vec<std::process::Child>);
    impl Drop for Children {
        fn drop(&mut self) {
            for child in &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let _children = Children(children);
    let manager = Manager::RequestAsync()?.get()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
    let sessions = loop {
        let sessions: Vec<_> = manager
            .GetSessions()?
            .into_iter()
            .filter(|s| {
                s.SourceAppUserModelId()
                    .is_ok_and(|id| id.to_string().starts_with(&prefix))
            })
            .collect();
        if sessions.len() == 2 {
            break sessions;
        }
        if std::time::Instant::now() > deadline {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                "Synthetic sessions did not appear; no owner session was read or controlled.",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    for session in &sessions {
        let properties = session.TryGetMediaPropertiesAsync()?.get()?;
        assert_eq!(properties.Title()?.to_string(), "Aurora synthetic track");
        assert_eq!(properties.Artist()?.to_string(), "Disposable fixture");
        assert!(properties.Thumbnail()?.OpenReadAsync()?.get()?.Size()? > 0);
        assert!(session.TryPauseAsync()?.get()?);
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert_eq!(session.GetPlaybackInfo()?.PlaybackStatus()?,windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused);
        assert!(session.TryPlayAsync()?.get()?);
        if session
            .SourceAppUserModelId()?
            .to_string()
            .ends_with(".two")
        {
            assert!(!session.GetPlaybackInfo()?.Controls()?.IsNextEnabled()?);
            assert!(!session.GetPlaybackInfo()?.Controls()?.IsPreviousEnabled()?);
            continue;
        }
        assert!(session.TrySkipNextAsync()?.get()?);
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert_eq!(
            session
                .TryGetMediaPropertiesAsync()?
                .get()?
                .Title()?
                .to_string(),
            "Aurora synthetic next"
        );
        assert!(session.TrySkipPreviousAsync()?.get()?);
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert_eq!(
            session
                .TryGetMediaPropertiesAsync()?
                .get()?
                .Title()?
                .to_string(),
            "Aurora synthetic track"
        );
    }
    drop(_children);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if !manager.GetSessions()?.into_iter().any(|s| {
            s.SourceAppUserModelId()
                .is_ok_and(|id| id.to_string().starts_with(&prefix))
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Synthetic sessions did not disappear"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!(
        "PASS: two isolated Windows media sessions appear/disappear; synthetic title/artist/art; pause/play and supported next/previous; unsupported-control flags. Volume changed only on silent test players, never master volume."
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows media acceptance requires Windows.");
}
