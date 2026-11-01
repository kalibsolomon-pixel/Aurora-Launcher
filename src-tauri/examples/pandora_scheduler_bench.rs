//! Controlled scheduling-only comparison. No internet throughput claim.
use aurora_launcher_lib::transfer::{settled_each, settled_window};
use std::time::{Duration, Instant};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let items: Vec<_> = (0..64).collect();
    for run in 0..6 {
        for completion in if run % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let started = Instant::now();
            let acquire = |item: &i32| {
                let delay = if *item == 0 { 400 } else { 20 };
                async move {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    Ok::<_, ()>(())
                }
            };
            if completion {
                settled_each(&items, 8, || true, acquire, |_, ()| {})
                    .await
                    .unwrap();
            } else {
                settled_window(&items, 8, || true, acquire, |_, ()| {})
                    .await
                    .unwrap();
            }
            println!(
                "{{\"run\":{run},\"completionOrder\":{completion},\"elapsedMs\":{}}}",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}
