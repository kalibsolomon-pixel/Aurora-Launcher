//! Cooperative cancellation for a single content installation, shared by both
//! hosts. Never abort/drop an installation future: active writes settle, cache
//! objects survive, and a commit boundary runs to its actual result.
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::install::processors::{CancelHandle, CancelSignal, cancellation_channel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Downloading,
    Verifying,
    Installing,
    Cancelling,
}

struct State {
    phase: Phase,
    commit_depth: usize,
    observed: bool,
    /// The operation's own named step (for example a `modpack-progress`
    /// phase), so a host can label this exact job among concurrent ones.
    stage: Option<&'static str>,
}

#[derive(Clone)]
pub struct Control {
    handle: CancelHandle,
    signal: CancelSignal,
    state: Arc<Mutex<State>>,
    /// Body bytes received by this job's downloads (all attempts).
    streamed: Arc<AtomicU64>,
    /// Whether cancellation also stops this job's active downloads between
    /// streamed chunks ([`transfer_checkpoint`]). Off for installations,
    /// whose active transfers settle into the cache for other consumers.
    interrupt_transfers: bool,
}

impl Default for Control {
    fn default() -> Self {
        let (handle, signal) = cancellation_channel();
        Self {
            handle,
            signal,
            state: Arc::new(Mutex::new(State {
                phase: Phase::Queued,
                commit_depth: 0,
                observed: false,
                stage: None,
            })),
            streamed: Arc::new(AtomicU64::new(0)),
            interrupt_transfers: false,
        }
    }
}

impl Control {
    /// A job whose cancellation also stops its active downloads at the next
    /// streamed chunk. For read-only work such as a review, whose partial
    /// download is untrusted staging with nothing to settle: the staging
    /// file is removed and nothing is promoted.
    pub fn interrupting_transfers() -> Self {
        Self {
            interrupt_transfers: true,
            ..Self::default()
        }
    }

    pub fn cancel(&self) {
        // Serialize the request against entry to commit. A request after entry
        // does not interrupt it, including nested provider transactions.
        let mut state = self.state.lock().expect("content job state");
        self.handle.cancel();
        state.phase = Phase::Cancelling;
    }

    pub fn phase(&self) -> Phase {
        self.state.lock().expect("content job state").phase
    }

    /// The latest step reported through [`set_stage`] inside this job.
    pub fn stage(&self) -> Option<&'static str> {
        self.state.lock().expect("content job state").stage
    }

    /// Download body bytes received inside this job so far, counting every
    /// attempt (a retried transfer counts again). Progress display only.
    pub fn streamed_bytes(&self) -> u64 {
        self.streamed.load(Ordering::Relaxed)
    }

    pub fn cancellation_observed(&self) -> bool {
        self.state.lock().expect("content job state").observed
    }

    pub async fn run<T>(&self, work: impl Future<Output = T>) -> T {
        CURRENT.scope(self.clone(), work).await
    }
}

tokio::task_local! { static CURRENT: Control; }

#[derive(Debug)]
pub struct Cancelled;

pub fn checkpoint() -> Result<(), Cancelled> {
    CURRENT
        .try_with(|control| {
            let mut state = control.state.lock().expect("content job state");
            if control.signal.is_cancelled() && state.commit_depth == 0 {
                state.observed = true;
                Err(Cancelled)
            } else {
                Ok(())
            }
        })
        .unwrap_or(Ok(()))
}

/// [`checkpoint`] between streamed download chunks: only a job created with
/// [`Control::interrupting_transfers`] observes cancellation here. Other
/// jobs' active transfers run to their verified result, as before.
pub fn transfer_checkpoint() -> Result<(), Cancelled> {
    if CURRENT
        .try_with(|control| control.interrupt_transfers)
        .unwrap_or(false)
    {
        checkpoint()
    } else {
        Ok(())
    }
}

pub fn phase(phase: Phase) {
    let _ = CURRENT.try_with(|control| {
        let mut state = control.state.lock().expect("content job state");
        if !control.signal.is_cancelled() {
            state.phase = phase;
        }
    });
}

/// Adds received download bytes to the running job's counter. Outside a
/// job scope it does nothing.
pub fn record_streamed(bytes: u64) {
    let _ = CURRENT.try_with(|control| {
        control.streamed.fetch_add(bytes, Ordering::Relaxed);
    });
}

/// Records the running job's named step. Outside a job scope it does nothing.
pub fn set_stage(stage: &'static str) {
    let _ = CURRENT.try_with(|control| {
        control.state.lock().expect("content job state").stage = Some(stage);
    });
}

/// A synchronous/async commit shield. It does not own paths or cleanup policy;
/// existing domain transactions retain that authority. Cancellation wins if it
/// arrived before entry, otherwise the entire transaction settles.
pub struct Commit(Option<Control>);
pub fn commit() -> Result<Commit, Cancelled> {
    let control = CURRENT.try_with(Clone::clone).ok();
    if let Some(control) = &control {
        let mut state = control.state.lock().expect("content job state");
        if control.signal.is_cancelled() && state.commit_depth == 0 {
            state.observed = true;
            return Err(Cancelled);
        }
        state.commit_depth += 1;
        if !control.signal.is_cancelled() {
            state.phase = Phase::Installing;
        }
    }
    Ok(Commit(control))
}
impl Drop for Commit {
    fn drop(&mut self) {
        if let Some(control) = &self.0 {
            control
                .state
                .lock()
                .expect("content job state")
                .commit_depth -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_before_commit_is_observed_but_commit_is_noninterruptible() {
        let control = Control::default();
        control
            .run(async {
                let commit = commit().unwrap();
                control.cancel();
                assert_eq!(control.phase(), Phase::Cancelling);
                assert!(checkpoint().is_ok());
                assert!(super::commit().is_ok());
                assert!(!control.cancellation_observed());
                drop(commit);
                assert!(checkpoint().is_err());
                assert!(super::commit().is_err());
            })
            .await;
        assert!(control.cancellation_observed());
    }

    #[tokio::test]
    async fn jobs_are_independent_and_retry_has_a_fresh_control() {
        let cancelled = Control::default();
        cancelled.cancel();
        let other = Control::default();
        let (a, b) = tokio::join!(
            cancelled.run(async { checkpoint() }),
            other.run(async { checkpoint() })
        );
        assert!(a.is_err());
        assert!(b.is_ok());
        assert!(Control::default().run(async { checkpoint() }).await.is_ok());
        assert!(checkpoint().is_ok());
    }

    #[tokio::test]
    async fn a_stage_belongs_to_its_own_job_only() {
        let (first, second) = (Control::default(), Control::default());
        tokio::join!(
            first.run(async { set_stage("installingGame") }),
            second.run(async { set_stage("downloading") })
        );
        set_stage("outside");
        assert_eq!(first.stage(), Some("installingGame"));
        assert_eq!(second.stage(), Some("downloading"));
        assert_eq!(Control::default().stage(), None);
    }

    #[tokio::test]
    async fn streamed_bytes_belong_to_their_own_job_only() {
        let (first, second) = (Control::default(), Control::default());
        tokio::join!(
            first.run(async {
                record_streamed(10);
                record_streamed(5);
            }),
            second.run(async { record_streamed(7) })
        );
        record_streamed(100);
        assert_eq!(first.streamed_bytes(), 15);
        assert_eq!(second.streamed_bytes(), 7);
        assert_eq!(Control::default().streamed_bytes(), 0);
    }

    #[tokio::test]
    async fn only_an_interrupting_job_observes_cancellation_between_chunks() {
        let ordinary = Control::default();
        ordinary.cancel();
        assert!(ordinary.run(async { transfer_checkpoint() }).await.is_ok());
        assert!(!ordinary.cancellation_observed());
        let review = Control::interrupting_transfers();
        assert!(review.run(async { transfer_checkpoint() }).await.is_ok());
        review.cancel();
        assert!(review.run(async { transfer_checkpoint() }).await.is_err());
        assert!(review.cancellation_observed());
        // Its commit shield still wins, and outside any job nothing fails.
        let shielded = Control::interrupting_transfers();
        shielded
            .run(async {
                let _commit = commit().unwrap();
                shielded.cancel();
                assert!(transfer_checkpoint().is_ok());
            })
            .await;
        assert!(transfer_checkpoint().is_ok());
    }
}
