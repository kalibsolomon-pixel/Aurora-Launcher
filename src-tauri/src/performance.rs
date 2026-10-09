//! P0.2 research probes. Ordinary builds compile these calls to no-ops.
//! No probe accepts strings, paths, identities, arguments, errors or secrets.
use std::future::Future;

#[cfg_attr(not(feature = "performance-p0-2"), allow(dead_code))]
#[derive(Clone, Copy)]
#[repr(u16)]
pub(crate) enum Event {
    Initialization = 1,
    RuntimeStatus = 10,
    Readiness = 11,
    PlayPreparation = 12,
    UnrelatedIpc = 13,
    InstanceSelection = 14,
    GameIntegrity = 20,
    GameMetadata = 21,
    RuntimeMetadata = 22,
    ModInventory = 23,
    RuntimeIntegrity = 24,
    JavaDiagnostic = 25,
    Session = 26,
    SessionExchange = 27,
    SessionLockWait = 28,
    FinalContentLock = 29,
    FinalLockedValidation = 30,
    JavaSpawn = 31,
    BlockingQueue = 32,
    BlockingJob = 33,
    RequestDispatch = 34,
    ModInventoryCommand = 35,
    MojangMetadataHttp = 36,
    FabricMetadataHttp = 37,
    RuntimeMetadataHttp = 38,
}

/// Closed frontend vocabulary; numeric inputs carry only ephemeral counters.
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FrontendEvent {
    Initialize,
    RuntimeRequest,
    RuntimeComplete,
    ReadinessRequest,
    ReadinessComplete,
    Checking,
    Ready,
    ReadinessDiscarded,
    PlayRequest,
    PlayComplete,
    InstanceGeneration,
    AccountGeneration,
    IpcRequest,
    IpcComplete,
    FrontendOverflow,
}

#[cfg_attr(not(feature = "performance-p0-2"), allow(dead_code))]
#[derive(Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Probe {
    correlation: u32,
    instance_generation: u32,
    account_generation: u32,
}

#[derive(serde::Serialize)]
#[serde(untagged)]
pub(crate) enum ProbeError {
    #[cfg(feature = "performance-p0-2")]
    Authoritative(crate::application::CommandError),
    #[cfg(not(feature = "performance-p0-2"))]
    Disabled,
}

// Separate research entry points keep all ordinary command DTOs/signatures
// unchanged. Each delegates to exactly the same authoritative implementation.
#[tauri::command]
pub(crate) async fn performance_readiness(
    app: tauri::AppHandle,
    request: crate::application::LaunchReadinessRequest,
    probe: Probe,
) -> Result<crate::application::PlayReadinessDto, ProbeError> {
    #[cfg(feature = "performance-p0-2")]
    {
        enabled::measure_context(probe, crate::application::get_play_readiness(app, request))
            .await
            .map_err(ProbeError::Authoritative)
    }
    #[cfg(not(feature = "performance-p0-2"))]
    {
        let _ = (app, request, probe);
        Err(ProbeError::Disabled)
    }
}
#[tauri::command]
pub(crate) async fn performance_runtime_status(
    app: tauri::AppHandle,
    request: crate::application::ValidateInstalledGameRequest,
    probe: Probe,
) -> Result<crate::application::RuntimeStatusDto, ProbeError> {
    #[cfg(feature = "performance-p0-2")]
    {
        enabled::measure_context(
            probe,
            crate::application::get_instance_runtime_status(app, request),
        )
        .await
        .map_err(ProbeError::Authoritative)
    }
    #[cfg(not(feature = "performance-p0-2"))]
    {
        let _ = (app, request, probe);
        Err(ProbeError::Disabled)
    }
}
#[tauri::command]
pub(crate) async fn performance_play(
    app: tauri::AppHandle,
    request: crate::application::PlayRequest,
    probe: Probe,
) -> Result<crate::application::LaunchProcessDto, ProbeError> {
    #[cfg(feature = "performance-p0-2")]
    {
        enabled::measure_context(probe, crate::application::play_instance(app, request))
            .await
            .map_err(ProbeError::Authoritative)
    }
    #[cfg(not(feature = "performance-p0-2"))]
    {
        let _ = (app, request, probe);
        Err(ProbeError::Disabled)
    }
}

#[tauri::command]
pub(crate) fn performance_mark(
    event: FrontendEvent,
    correlation: u32,
    instance_generation: u32,
    account_generation: u32,
    client_ms: f64,
) -> Result<(), ()> {
    #[cfg(feature = "performance-p0-2")]
    {
        if !client_ms.is_finite() || !(0.0..=86_400_000.0).contains(&client_ms) {
            return Err(());
        }
        enabled::frontend(
            event,
            correlation,
            instance_generation,
            account_generation,
            client_ms,
        );
        Ok(())
    }
    #[cfg(not(feature = "performance-p0-2"))]
    {
        let _ = (
            event,
            correlation,
            instance_generation,
            account_generation,
            client_ms,
        );
        Err(())
    }
}

#[cfg(not(feature = "performance-p0-2"))]
pub(crate) fn measure<F: Future>(_: Event, future: F) -> F {
    future
}
#[cfg(not(feature = "performance-p0-2"))]
pub(crate) struct DisabledScope;
#[cfg(not(feature = "performance-p0-2"))]
pub(crate) fn scope(_: Event) -> DisabledScope {
    DisabledScope
}
#[cfg(not(feature = "performance-p0-2"))]
pub(crate) fn hash() {}
#[cfg(not(feature = "performance-p0-2"))]
pub(crate) fn flush() {}
#[cfg(not(feature = "performance-p0-2"))]
pub(crate) fn queued<F: FnOnce() -> T, T>(job: F) -> F {
    job
}

#[cfg(feature = "performance-p0-2")]
pub(crate) use enabled::{flush, hash, measure, queued, scope};

#[cfg(feature = "performance-p0-2")]
mod enabled {
    use super::{Event, FrontendEvent, Future, Probe};
    use std::cell::RefCell;
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::task::{Context as TaskContext, Poll};
    use std::time::Instant;

    const LIMIT: usize = 16_384;
    #[derive(serde::Serialize)]
    struct Record {
        event: u16,
        edge: u8,
        id: u64,
        parent: u64,
        us: u64,
        hash_us: u64,
        hash_calls: u64,
        correlation: u32,
        instance_generation: u32,
        account_generation: u32,
        client_us: u64,
    }
    #[derive(Default, serde::Serialize)]
    struct Buffer {
        records: Vec<Record>,
        dropped: u64,
    }
    impl Buffer {
        fn push(&mut self, record: Record) {
            if self.records.len() < LIMIT {
                self.records.push(record);
            } else {
                self.dropped = self.dropped.saturating_add(1);
            }
        }
    }
    struct State {
        start: Instant,
        next: AtomicU64,
        buffer: Mutex<Buffer>,
        output: Mutex<File>,
    }
    fn state() -> Option<&'static State> {
        static STATE: OnceLock<Option<State>> = OnceLock::new();
        STATE
            .get_or_init(|| {
                // Explicit opt-in AND compile-time feature. Exclusive output creation
                // refuses existing evidence; paths never enter a record or diagnostic.
                let root = std::path::PathBuf::from(std::env::var_os("AURORA_P0_2_CAPTURE_ROOT")?);
                let root = root.canonicalize().ok()?;
                let temporary = std::env::temp_dir().canonicalize().ok()?;
                let first = root.strip_prefix(&temporary).ok()?.components().next()?;
                if !first.as_os_str().to_str()?.starts_with("aurora-p0-2-") {
                    return None;
                }
                let output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(root.join("native-trace.json"))
                    .ok()?;
                Some(State {
                    start: Instant::now(),
                    next: AtomicU64::new(1),
                    buffer: Mutex::new(Buffer {
                        records: Vec::with_capacity(LIMIT),
                        dropped: 0,
                    }),
                    output: Mutex::new(output),
                })
            })
            .as_ref()
    }
    #[derive(Default)]
    struct Counters {
        nanos: AtomicU64,
        calls: AtomicU64,
    }
    #[derive(Clone)]
    struct Context {
        id: u64,
        counters: Arc<Counters>,
        probe: Probe,
    }
    thread_local! { static CURRENT: RefCell<Option<Context>> = const { RefCell::new(None) }; }
    struct Restore(Option<Context>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|c| {
                c.replace(self.0.take());
            });
        }
    }
    fn enter(context: Option<Context>) -> Restore {
        Restore(CURRENT.with(|c| c.replace(context)))
    }
    fn current() -> Option<Context> {
        CURRENT.with(|c| c.borrow().clone())
    }
    struct Sample {
        context: Context,
        parent: u64,
        event: Event,
        complete: bool,
    }
    impl Sample {
        fn new(event: Event) -> Option<Self> {
            let s = state()?;
            let result = Self {
                context: Context {
                    id: s.next.fetch_add(1, Ordering::Relaxed),
                    counters: Arc::default(),
                    probe: current().map_or(Probe::default(), |c| c.probe),
                },
                parent: current().map_or(0, |c| c.id),
                event,
                complete: false,
            };
            result.record(1);
            Some(result)
        }
        fn record(&self, edge: u8) {
            if let Some(s) = state() {
                s.buffer
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(Record {
                        event: self.event as u16,
                        edge,
                        id: self.context.id,
                        parent: self.parent,
                        us: s.start.elapsed().as_micros() as u64,
                        hash_us: self.context.counters.nanos.load(Ordering::Relaxed) / 1000,
                        hash_calls: self.context.counters.calls.load(Ordering::Relaxed),
                        correlation: self.context.probe.correlation,
                        instance_generation: self.context.probe.instance_generation,
                        account_generation: self.context.probe.account_generation,
                        client_us: 0,
                    });
            }
        }
    }
    impl Drop for Sample {
        fn drop(&mut self) {
            self.record(if self.complete { 2 } else { 3 });
        }
    }
    pub(crate) struct Scope {
        _restore: Restore,
        _sample: Option<Sample>,
    }
    /// Synchronous regions only. Never hold this TLS guard across an await.
    pub(crate) fn scope(event: Event) -> Scope {
        let mut sample = Sample::new(event);
        if let Some(s) = &mut sample {
            s.complete = true;
        }
        let restore = enter(sample.as_ref().map(|s| s.context.clone()));
        Scope {
            _restore: restore,
            _sample: sample,
        }
    }
    struct Measured<F> {
        future: Pin<Box<F>>,
        sample: Option<Sample>,
        event: Event,
        started: bool,
        probe: Option<Probe>,
    }
    impl<F: Future> Future for Measured<F> {
        type Output = F::Output;
        fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            if !this.started {
                this.started = true;
                // The dispatch scope owns only numeric correlation metadata.
                let _restore = this.probe.map(|probe| {
                    enter(Some(Context {
                        id: 0,
                        counters: Arc::default(),
                        probe,
                    }))
                });
                this.sample = Sample::new(this.event);
            }
            // Install/restore on EACH poll: pending futures must not contaminate
            // another request on the same executor thread or lose correlation
            // when resumed on a different thread.
            let _restore = enter(this.sample.as_ref().map(|s| s.context.clone()));
            let result = this.future.as_mut().poll(cx);
            if result.is_ready() {
                if let Some(s) = &mut this.sample {
                    s.complete = true;
                }
                this.sample.take();
            }
            result
        }
    }
    pub(crate) fn measure<F: Future>(event: Event, future: F) -> impl Future<Output = F::Output> {
        Measured {
            future: Box::pin(future),
            sample: None,
            event,
            started: false,
            probe: None,
        }
    }
    pub(super) fn measure_context<F: Future>(
        probe: Probe,
        future: F,
    ) -> impl Future<Output = F::Output> {
        Measured {
            future: Box::pin(future),
            sample: None,
            event: Event::RequestDispatch,
            started: false,
            probe: Some(probe),
        }
    }
    pub(crate) struct Hash {
        start: Option<Instant>,
        context: Option<Context>,
    }
    pub(crate) fn hash() -> Hash {
        let context = current();
        Hash {
            start: context.as_ref().map(|_| Instant::now()),
            context,
        }
    }
    impl Drop for Hash {
        fn drop(&mut self) {
            if let (Some(start), Some(context)) = (&self.start, &self.context) {
                context
                    .counters
                    .nanos
                    .fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
                context.counters.calls.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    pub(crate) fn queued<F: FnOnce() -> T, T>(job: F) -> impl FnOnce() -> T {
        let mut wait = Sample::new(Event::BlockingQueue);
        let context = current();
        move || {
            if let Some(s) = &mut wait {
                s.complete = true;
            }
            drop(wait);
            let _restore = enter(context);
            let _scope = scope(Event::BlockingJob);
            job()
        }
    }
    pub(super) fn frontend(
        event: FrontendEvent,
        correlation: u32,
        instance_generation: u32,
        account_generation: u32,
        client_ms: f64,
    ) {
        if let Some(s) = state() {
            s.buffer
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(Record {
                    event: 100 + event as u16,
                    edge: 0,
                    id: 0,
                    parent: 0,
                    us: s.start.elapsed().as_micros() as u64,
                    hash_us: 0,
                    hash_calls: 0,
                    correlation,
                    instance_generation,
                    account_generation,
                    client_us: (client_ms * 1000.0) as u64,
                });
        }
    }
    pub(crate) fn flush() {
        if let Some(s) = state() {
            // One exit-time write. No I/O occurs in measured validation loops.
            static FLUSHED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if FLUSHED.swap(true, Ordering::Relaxed) {
                return;
            }
            if let Ok(buffer) = s.buffer.lock() {
                if let Ok(mut output) = s.output.lock() {
                    let _ = serde_json::to_writer(&mut *output, &*buffer);
                    let _ = output.flush();
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn record() -> Record {
            Record {
                event: 20,
                edge: 1,
                id: 1,
                parent: 0,
                us: 1,
                hash_us: 0,
                hash_calls: 0,
                correlation: 0,
                instance_generation: 0,
                account_generation: 0,
                client_us: 0,
            }
        }
        #[test]
        fn overflow_is_bounded_and_serialization_is_numeric_only() {
            let mut b = Buffer::default();
            for _ in 0..LIMIT + 100 {
                b.push(record());
            }
            assert_eq!(b.records.len(), LIMIT);
            assert_eq!(b.dropped, 100);
            let json = serde_json::to_value(&b).unwrap();
            for r in json["records"].as_array().unwrap() {
                assert_eq!(r.as_object().unwrap().len(), 11);
                assert!(
                    r.as_object()
                        .unwrap()
                        .values()
                        .all(serde_json::Value::is_u64)
                );
            }
        }
        #[test]
        fn vocabulary_rejects_private_payloads() {
            for raw in ["\"accountId\"", "\"token\"", "\"C:\\\\private\"", "123"] {
                assert!(serde_json::from_str::<FrontendEvent>(raw).is_err());
            }
        }
        #[test]
        fn context_restores_and_hash_counters_stay_with_their_owner() {
            let counters = Arc::new(Counters::default());
            let _restore = enter(Some(Context {
                id: 42,
                counters: counters.clone(),
                probe: Probe::default(),
            }));
            {
                let _nested = enter(Some(Context {
                    id: 43,
                    counters: Arc::default(),
                    probe: Probe::default(),
                }));
                assert_eq!(current().unwrap().id, 43);
            }
            assert_eq!(current().unwrap().id, 42);
            drop(hash());
            assert_eq!(counters.calls.load(Ordering::Relaxed), 1);
        }

        #[test]
        fn pending_future_restores_executor_context_and_resumes_on_another_thread() {
            struct Wake;
            impl std::task::Wake for Wake {
                fn wake(self: Arc<Self>) {}
            }
            let counters = Arc::new(Counters::default());
            let observed = counters.clone();
            let mut pending = true;
            let future = std::future::poll_fn(move |_| {
                assert_eq!(current().unwrap().id, 72);
                assert_eq!(current().unwrap().probe.correlation, 91);
                drop(hash());
                if std::mem::take(&mut pending) {
                    Poll::Pending
                } else {
                    Poll::Ready(7)
                }
            });
            // Inject a sample without enabling file output in the test process.
            let mut measured = Box::pin(Measured {
                future: Box::pin(future),
                sample: Some(Sample {
                    context: Context {
                        id: 72,
                        counters,
                        probe: Probe {
                            correlation: 91,
                            ..Probe::default()
                        },
                    },
                    parent: 0,
                    event: Event::Readiness,
                    complete: false,
                }),
                event: Event::Readiness,
                started: true,
                probe: None,
            });
            let waker = std::task::Waker::from(Arc::new(Wake));
            assert!(
                measured
                    .as_mut()
                    .poll(&mut TaskContext::from_waker(&waker))
                    .is_pending()
            );
            assert!(current().is_none());
            std::thread::spawn(move || {
                assert!(current().is_none());
                let waker = std::task::Waker::from(Arc::new(Wake));
                assert_eq!(
                    measured.as_mut().poll(&mut TaskContext::from_waker(&waker)),
                    Poll::Ready(7)
                );
                assert!(current().is_none());
            })
            .join()
            .unwrap();
            assert_eq!(observed.calls.load(Ordering::Relaxed), 2);
        }
    }
}
