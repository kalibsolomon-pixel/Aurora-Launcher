//! Process-wide transfer coordination for the verified artifact cache.
//!
//! Three small mechanisms, applied by `cache` around every expected-digest
//! acquisition, so concurrent installs into different instances share one
//! bounded network budget:
//!
//! - **Single flight.** One in-process owner per artifact identity. A second
//!   request for the same digest waits, then finds a verified cache hit
//!   (re-hashed as usual) instead of downloading identical bytes. Dropping
//!   the owner (cancellation) releases the waiters; the next one acquires
//!   the artifact itself, so one consumer's cancellation never strands
//!   another that still needs it.
//! - **Transfer permits.** A global and a per-host bound on simultaneous
//!   network transfers. Cache hits never take a permit.
//! - **Bounded retry.** Transient transport failures and retryable HTTP
//!   statuses are retried with exponential backoff and jitter. Integrity
//!   failures are never retried into success: each attempt verifies from a
//!   fresh staging file, and digest mismatches fail immediately.
//!
//! Cross-process exclusion is unchanged: store promotion stays atomic and
//! content-addressed, so two processes can at worst both download.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::{OwnedMutexGuard, OwnedSemaphorePermit, Semaphore};

use crate::downloads::DownloadError;

/// Simultaneous network transfers across the whole process.
pub const GLOBAL_TRANSFER_LIMIT: usize = 24;
/// Simultaneous transfers to one host. Below the global limit so a single
/// CDN never takes every slot while another install waits on a second host.
pub const PER_HOST_TRANSFER_LIMIT: usize = 16;
/// Total attempts for one artifact (the first try plus retries).
pub const MAX_ATTEMPTS: u32 = 4;
const BASE_BACKOFF: Duration = Duration::from_millis(400);
const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(2);
const MAX_BACKOFF: Duration = Duration::from_secs(10);

/// Monotonic counters for diagnostics and acceptance measurements.
#[derive(Debug, Default)]
pub struct TransferStats {
    pub transfers_started: AtomicU64,
    pub transfers_succeeded: AtomicU64,
    pub retries: AtomicU64,
    pub deduplicated_waits: AtomicU64,
    pub bytes_downloaded: AtomicU64,
}

/// A point-in-time copy of [`TransferStats`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSnapshot {
    pub transfers_started: u64,
    pub transfers_succeeded: u64,
    pub retries: u64,
    pub deduplicated_waits: u64,
    pub bytes_downloaded: u64,
    pub active_transfers: u64,
}

struct Coordinator {
    global: Arc<Semaphore>,
    hosts: Mutex<HashMap<String, Arc<Semaphore>>>,
    flights: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    stats: TransferStats,
}

fn coordinator() -> &'static Coordinator {
    static COORDINATOR: OnceLock<Coordinator> = OnceLock::new();
    COORDINATOR.get_or_init(|| Coordinator {
        global: Arc::new(Semaphore::new(GLOBAL_TRANSFER_LIMIT)),
        hosts: Mutex::new(HashMap::new()),
        flights: Mutex::new(HashMap::new()),
        stats: TransferStats::default(),
    })
}

pub fn snapshot() -> TransferSnapshot {
    let c = coordinator();
    let s = &c.stats;
    TransferSnapshot {
        transfers_started: s.transfers_started.load(Ordering::Relaxed),
        transfers_succeeded: s.transfers_succeeded.load(Ordering::Relaxed),
        retries: s.retries.load(Ordering::Relaxed),
        deduplicated_waits: s.deduplicated_waits.load(Ordering::Relaxed),
        bytes_downloaded: s.bytes_downloaded.load(Ordering::Relaxed),
        active_transfers: (GLOBAL_TRANSFER_LIMIT - c.global.available_permits()) as u64,
    }
}

/// Exclusive in-process ownership of one artifact identity.
pub struct FlightGuard {
    key: String,
    _guard: OwnedMutexGuard<()>,
}

impl Drop for FlightGuard {
    fn drop(&mut self) {
        // Forget the slot once nobody else holds or awaits it, so the map
        // stays bounded by in-flight work rather than by history.
        let mut flights = coordinator()
            .flights
            .lock()
            .expect("flight map is not poisoned");
        if let Some(slot) = flights.get(&self.key) {
            // One reference in the map plus the one inside our guard.
            if Arc::strong_count(slot) <= 2 {
                flights.remove(&self.key);
            }
        }
    }
}

/// Waits until this process owns `key` (for example `sha1:<hex>`).
pub async fn single_flight(key: String) -> FlightGuard {
    let slot = {
        let mut flights = coordinator()
            .flights
            .lock()
            .expect("flight map is not poisoned");
        Arc::clone(
            flights
                .entry(key.clone())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
        )
    };
    let guard = match Arc::clone(&slot).try_lock_owned() {
        Ok(guard) => guard,
        Err(_) => {
            coordinator()
                .stats
                .deduplicated_waits
                .fetch_add(1, Ordering::Relaxed);
            slot.lock_owned().await
        }
    };
    FlightGuard { key, _guard: guard }
}

/// Held for the duration of one network transfer.
pub struct TransferPermit {
    _global: OwnedSemaphorePermit,
    _host: OwnedSemaphorePermit,
}

pub async fn permit(host: &str) -> TransferPermit {
    let c = coordinator();
    let host_semaphore = {
        let mut hosts = c.hosts.lock().expect("host map is not poisoned");
        Arc::clone(
            hosts
                .entry(host.to_ascii_lowercase())
                .or_insert_with(|| Arc::new(Semaphore::new(PER_HOST_TRANSFER_LIMIT))),
        )
    };
    // Host first: a request queued behind a saturated host does not hold a
    // global slot that another host's request could use.
    let host = host_semaphore
        .acquire_owned()
        .await
        .expect("transfer semaphores are never closed");
    let global = Arc::clone(&c.global)
        .acquire_owned()
        .await
        .expect("transfer semaphores are never closed");
    TransferPermit {
        _global: global,
        _host: host,
    }
}

/// Whether a failed attempt may succeed if repeated. Integrity failures
/// (digest mismatch, observed drift) and policy refusals are final.
pub fn is_transient(error: &DownloadError) -> bool {
    match error {
        DownloadError::Cancelled => false,
        DownloadError::Network(_) | DownloadError::Timeout(_) => true,
        DownloadError::HttpStatus { status } => {
            matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504)
        }
        // A truncated body surfaces as a size mismatch; the next attempt
        // verifies independently.
        DownloadError::SizeMismatch { .. } => true,
        DownloadError::StagingIo(_)
        | DownloadError::Redirect(_)
        | DownloadError::Sha256Mismatch { .. }
        | DownloadError::Sha1Mismatch { .. }
        | DownloadError::Sha512Mismatch { .. }
        | DownloadError::ObservedDigestDrift { .. } => false,
    }
}

/// Delay before attempt `attempt + 1` (attempt is 1-based). Exponential,
/// capped, with ±50% jitter so parallel retries do not synchronize.
pub fn backoff(attempt: u32, error: &DownloadError) -> Duration {
    let base = if matches!(error, DownloadError::HttpStatus { status: 429 | 503 }) {
        RATE_LIMIT_BACKOFF
    } else {
        BASE_BACKOFF
    };
    let exponential = base.saturating_mul(1u32 << (attempt.saturating_sub(1)).min(5));
    let capped = exponential.min(MAX_BACKOFF);
    let jitter = jitter_fraction();
    capped.mul_f64(0.5 + jitter)
}

fn jitter_fraction() -> f64 {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or_default();
    let mut x = nanos ^ SEQUENCE.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
    // splitmix64 finalizer: cheap, adequately spread for jitter.
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Runs one network transfer under a permit for `host`, retrying transient
/// failures. `attempt` must start from a fresh staging file each call and
/// clean it up on failure (the download functions already do).
pub async fn with_retry<T, F, Fut>(host: &str, mut attempt: F) -> Result<T, DownloadError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(T, u64), DownloadError>>,
{
    let stats = &coordinator().stats;
    let mut tries = 0u32;
    loop {
        crate::content_jobs::checkpoint().map_err(|_| DownloadError::Cancelled)?;
        crate::content_jobs::phase(crate::content_jobs::Phase::Downloading);
        tries += 1;
        let outcome = {
            let _permit = permit(host).await;
            crate::content_jobs::checkpoint().map_err(|_| DownloadError::Cancelled)?;
            stats.transfers_started.fetch_add(1, Ordering::Relaxed);
            attempt().await
        };
        match outcome {
            Ok((value, bytes)) => {
                stats.transfers_succeeded.fetch_add(1, Ordering::Relaxed);
                stats.bytes_downloaded.fetch_add(bytes, Ordering::Relaxed);
                return Ok(value);
            }
            Err(error) if tries < MAX_ATTEMPTS && is_transient(&error) => {
                crate::content_jobs::checkpoint().map_err(|_| DownloadError::Cancelled)?;
                stats.retries.fetch_add(1, Ordering::Relaxed);
                let delay = backoff(tries, &error);
                eprintln!(
                    "[aurora-launcher] transient download failure from {host} (attempt {tries}/{MAX_ATTEMPTS}); retrying in {} ms",
                    delay.as_millis()
                );
                tokio::time::sleep(delay).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Runs `acquire` over `items` with at most `width` in flight, delivering
/// successes to `accept` in input order. After the first failure, or once
/// `should_start` says no (cancellation), no further item starts; items
/// already in flight are awaited before the first error is returned, so no
/// transfer is dropped mid-write with an open staging file.
pub async fn settled_window<'a, T, R, E, Fut>(
    items: &'a [T],
    width: usize,
    should_start: impl Fn() -> bool,
    acquire: impl Fn(&'a T) -> Fut,
    mut accept: impl FnMut(&'a T, R),
) -> Result<(), E>
where
    Fut: Future<Output = Result<R, E>>,
{
    use futures_util::stream::{FuturesOrdered, StreamExt};

    let mut first_error = None;
    let mut pending = items.iter();
    let mut in_flight = FuturesOrdered::new();
    loop {
        while first_error.is_none() && in_flight.len() < width.max(1) && should_start() {
            let Some(item) = pending.next() else { break };
            let future = acquire(item);
            in_flight.push_back(async move { (item, future.await) });
        }
        let Some((item, outcome)) = in_flight.next().await else {
            break;
        };
        match outcome {
            Ok(value) if first_error.is_none() => accept(item, value),
            Ok(_) => {}
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[tokio::test]
    async fn settled_window_keeps_order_bounds_width_and_settles_after_failure() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let items: Vec<u32> = (0..20).collect();
        let mut seen = Vec::new();
        let result: Result<(), u32> = settled_window(
            &items,
            4,
            || true,
            |item| {
                let (active, peak, started) =
                    (Arc::clone(&active), Arc::clone(&peak), Arc::clone(&started));
                async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    // Later items finish first, so ordering is really tested.
                    tokio::time::sleep(Duration::from_millis(20 - *item as u64 % 7)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    if *item == 9 { Err(*item) } else { Ok(*item) }
                }
            },
            |_, value| seen.push(value),
        )
        .await;
        assert_eq!(result, Err(9));
        assert_eq!(seen, (0..9).collect::<Vec<_>>());
        assert!(peak.load(Ordering::SeqCst) <= 4);
        assert_eq!(
            active.load(Ordering::SeqCst),
            0,
            "every started item settled"
        );
        assert!(
            started.load(Ordering::SeqCst) < 20,
            "no new work after the failure"
        );
    }

    #[test]
    fn integrity_failures_are_never_transient() {
        for error in [
            DownloadError::Sha1Mismatch {
                expected: "a".into(),
                actual: "b".into(),
            },
            DownloadError::Sha256Mismatch {
                expected: "a".into(),
                actual: "b".into(),
            },
            DownloadError::HttpStatus { status: 404 },
            DownloadError::HttpStatus { status: 403 },
        ] {
            assert!(!is_transient(&error), "{error:?}");
        }
        assert!(is_transient(&DownloadError::HttpStatus { status: 503 }));
        assert!(is_transient(&DownloadError::HttpStatus { status: 429 }));
    }

    #[test]
    fn backoff_grows_is_capped_and_jittered() {
        let e = DownloadError::HttpStatus { status: 500 };
        for attempt in 1..=10 {
            let d = backoff(attempt, &e);
            assert!(d <= MAX_BACKOFF.mul_f64(1.5), "{d:?}");
            assert!(d >= BASE_BACKOFF.mul_f64(0.5), "{d:?}");
        }
        let limited = backoff(1, &DownloadError::HttpStatus { status: 429 });
        assert!(limited >= RATE_LIMIT_BACKOFF.mul_f64(0.5));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn retries_transient_failures_then_succeeds() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let result = with_retry("retry.test", move || {
            let c = Arc::clone(&c);
            async move {
                if c.fetch_add(1, Ordering::SeqCst) < 2 {
                    Err(DownloadError::HttpStatus { status: 500 })
                } else {
                    Ok((7u8, 1))
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 7);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn integrity_failures_are_not_retried() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let result: Result<u8, _> = with_retry("integrity.test", move || {
            let c = Arc::clone(&c);
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(DownloadError::Sha1Mismatch {
                    expected: "a".into(),
                    actual: "b".into(),
                })
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn single_flight_serializes_one_identity_and_releases_on_drop() {
        let first = single_flight("sha1:flight-test".into()).await;
        let waiter = tokio::spawn(async {
            let _second = single_flight("sha1:flight-test".into()).await;
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!waiter.is_finished(), "the second request must wait");
        // Cancellation of the owner releases the waiter.
        drop(first);
        tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .unwrap()
            .unwrap();
        assert!(
            !coordinator()
                .flights
                .lock()
                .unwrap()
                .contains_key("sha1:flight-test")
        );
        // A different identity never waits on this one.
        let _a = single_flight("sha1:flight-a".into()).await;
        let _b = tokio::time::timeout(
            Duration::from_millis(200),
            single_flight("sha1:flight-b".into()),
        )
        .await
        .expect("independent identities proceed concurrently");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn per_host_permits_bound_concurrency() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..(PER_HOST_TRANSFER_LIMIT * 2) {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            tasks.push(tokio::spawn(async move {
                let _p = permit("bounded.test").await;
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(10)).await;
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= PER_HOST_TRANSFER_LIMIT);
    }
}
