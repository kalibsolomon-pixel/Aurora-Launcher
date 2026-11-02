//! Cooperative cancellation for a single content installation, shared by both
//! hosts. Never abort/drop an installation future: active writes settle, cache
//! objects survive, and a commit boundary runs to its actual result.
use std::future::Future;
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
}

#[derive(Clone)]
pub struct Control {
    handle: CancelHandle,
    signal: CancelSignal,
    state: Arc<Mutex<State>>,
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
            })),
        }
    }
}

impl Control {
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

pub fn phase(phase: Phase) {
    let _ = CURRENT.try_with(|control| {
        let mut state = control.state.lock().expect("content job state");
        if !control.signal.is_cancelled() {
            state.phase = phase;
        }
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
}
