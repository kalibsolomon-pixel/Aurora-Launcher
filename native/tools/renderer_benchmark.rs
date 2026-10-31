//! Standalone raster benchmark (no UI/network/core startup). Compile with
//! `rustc --edition=2024 -O native/tools/renderer_benchmark.rs -o <temp.exe>`.
//! Input directory contains decoded classic.rgba, slim.rgba (64×64) and
//! cape.rgba (64×32). Inputs are synthetic acceptance fixtures, never accounts.
#![allow(dead_code)]
mod api {
    pub struct HeadAvatar {
        pub rgba: Vec<u8>,
        pub skin_rgba: Vec<u8>,
        pub skin_height: u32,
        pub model: String,
    }
    pub struct Texture {
        pub width: u32,
        pub height: u32,
        pub rgba: Vec<u8>,
    }
}
#[path = "../src/player.rs"]
mod player;
fn main() {
    let root = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("synthetic fixture directory"),
    );
    let cape = api::Texture {
        width: 64,
        height: 32,
        rgba: std::fs::read(root.join("cape.rgba")).unwrap(),
    };
    assert_eq!(cape.rgba.len(), 64 * 32 * 4);
    for model in ["classic", "slim"] {
        let skin = api::HeadAvatar {
            rgba: vec![],
            model: model.into(),
            skin_height: 64,
            skin_rgba: std::fs::read(root.join(format!("{model}.rgba"))).unwrap(),
        };
        assert_eq!(skin.skin_rgba.len(), 64 * 64 * 4);
        let mut times = Vec::with_capacity(600);
        for i in 0..630 {
            let start = std::time::Instant::now();
            std::hint::black_box(player::render(
                Some(&skin),
                Some(&cape),
                i as f32 * 0.03,
                0.1,
                1.,
                true,
            ));
            if i >= 30 {
                times.push(start.elapsed().as_secs_f64() * 1000.);
            }
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        times.sort_by(f64::total_cmp);
        println!(
            "{{\"model\":\"{model}\",\"frames\":600,\"mean_ms\":{mean:.4},\"p50_ms\":{:.4},\"p95_ms\":{:.4},\"max_ms\":{:.4}}}",
            times[300], times[570], times[599]
        );
    }
}
