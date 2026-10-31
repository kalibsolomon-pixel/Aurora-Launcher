//! Local-only texture/renderer acceptance. Files stay in a marked temp root.
#![allow(dead_code)]
#[path = "../src/api.rs"]
mod api;
#[path = "../src/player.rs"]
mod player;
use aurora_launcher_lib::host::{Host, LAUNCHER_VERSION};
fn main() {
    let root = std::path::PathBuf::from(std::env::args_os().nth(1).expect("marked temp root"));
    assert_eq!(
        root.parent().unwrap().canonicalize().unwrap(),
        std::env::temp_dir().canonicalize().unwrap()
    );
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("aurora-native7-")
    );
    assert_eq!(
        std::fs::read(root.join("NATIVE7-DISPOSABLE.txt")).unwrap(),
        b"Native 7 disposable acceptance\n"
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let host = Host::native(
        Ok(root.join("managed")),
        LAUNCHER_VERSION,
        runtime.handle().clone(),
        |_, _| {},
    );
    runtime.block_on(async move {
        for model in ["classic", "slim"] {
            let bytes = std::fs::read(root.join(format!("{model}.png"))).unwrap();
            let imported = api::import_skin(
                host.clone(),
                format!("Synthetic {model} acceptance"),
                model.into(),
                bytes,
            )
            .await
            .unwrap();
            let id = imported["preset"]["id"].as_str().unwrap().to_owned();
            let skin = api::skin_preview(host.clone(), id).await.unwrap();
            let cape = api::Texture {
                width: 64,
                height: 32,
                rgba: std::fs::read(root.join("cape.rgba")).unwrap(),
            };
            for (label, yaw, with_cape) in [
                ("front", player::DEFAULT_YAW, false),
                ("back-cape", 2.8, true),
            ] {
                let pixels =
                    player::render(Some(&skin), with_cape.then_some(&cape), yaw, 0.1, 1., true);
                std::fs::write(root.join(format!("{model}-{label}.rgba")), pixels).unwrap();
            }
            let start = std::time::Instant::now();
            for i in 0..120 {
                std::hint::black_box(player::render(
                    Some(&skin),
                    Some(&cape),
                    i as f32 * 0.03,
                    0.1,
                    1.,
                    true,
                ));
            }
            println!(
                "{model}: 120 rotating frames, {} ms total, {:.3} ms/frame",
                start.elapsed().as_millis(),
                start.elapsed().as_secs_f64() * 1000. / 120.
            );
        }
        let bad = api::import_skin(
            host,
            "Malformed acceptance".into(),
            "classic".into(),
            b"not a PNG".to_vec(),
        )
        .await;
        assert!(bad.is_err());
        println!("Malformed PNG refused; no account calls made.");
    });
}
