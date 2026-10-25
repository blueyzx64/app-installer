//! Test doubles for the bus. Test-only.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use installer_application::CancelToken;
use installer_domain::BackendError;

use super::transaction::{CallError, PkBus, PkEvent, PkOp, PkTransaction};

pub enum Step {
    Event(PkEvent),
    /// Let the driver run other branches once.
    Yield,
    /// Fire the cancel token (a person pressing Cancel).
    CancelToken(CancelToken),
    AssertCancels(usize),
}

struct Inner {
    script: Mutex<VecDeque<Step>>,
    call_error: Option<CallError>,
    cancels: AtomicUsize,
    ops: Mutex<Vec<PkOp>>,
}

#[derive(Clone)]
pub struct FakeTx(Arc<Inner>);

impl FakeTx {
    pub fn new(script: Vec<Step>) -> Self {
        FakeTx(Arc::new(Inner {
            script: Mutex::new(script.into()),
            call_error: None,
            cancels: AtomicUsize::new(0),
            ops: Mutex::new(Vec::new()),
        }))
    }

    pub fn with_call_error(self, e: CallError) -> Self {
        let script = std::mem::take(&mut *self.0.script.lock().unwrap());
        FakeTx(Arc::new(Inner {
            script: Mutex::new(script),
            call_error: Some(e),
            cancels: AtomicUsize::new(0),
            ops: Mutex::new(Vec::new()),
        }))
    }

    pub fn cancels(&self) -> usize {
        self.0.cancels.load(Ordering::SeqCst)
    }

    pub fn ops(&self) -> Vec<PkOp> {
        self.0.ops.lock().unwrap().clone()
    }
}

impl PkTransaction for FakeTx {
    async fn start(&self, op: PkOp) -> Result<(), CallError> {
        self.0.ops.lock().unwrap().push(op);
        match &self.0.call_error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }

    async fn cancel(&self) {
        self.0.cancels.fetch_add(1, Ordering::SeqCst);
    }

    async fn next_event(&self) -> Option<PkEvent> {
        loop {
            let step = self.0.script.lock().unwrap().pop_front()?;
            match step {
                Step::Event(e) => return Some(e),
                Step::Yield => tokio::task::yield_now().await,
                Step::CancelToken(t) => t.cancel(),
                Step::AssertCancels(n) => assert_eq!(self.cancels(), n),
            }
        }
    }
}

/// Hands out the queued transactions, one per `begin()`.
pub struct FakeBus {
    queue: Mutex<VecDeque<FakeTx>>,
    pub available: bool,
}

impl FakeBus {
    pub fn new(txs: Vec<FakeTx>) -> Self {
        FakeBus {
            queue: Mutex::new(txs.into()),
            available: true,
        }
    }
}

impl PkBus for FakeBus {
    type Tx = FakeTx;

    async fn begin(&self) -> Result<FakeTx, BackendError> {
        self.queue
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| BackendError::Unavailable("no more fake transactions".into()))
    }

    async fn is_available(&self) -> bool {
        self.available
    }
}
