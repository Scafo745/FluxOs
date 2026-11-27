//! In-process event bus. Every event is broadcast to SSE subscribers and kept
//! in a ring buffer, so a UI that connects after boot can replay the boot log.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

use crate::util::now;

const BUFFER: usize = 512;

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    pub kind: String,
    pub data: Value,
}

pub struct EventBus {
    tx: broadcast::Sender<Event>,
    buf: Mutex<VecDeque<Event>>,
    seq: AtomicU64,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx, buf: Mutex::new(VecDeque::with_capacity(BUFFER)), seq: AtomicU64::new(0) }
    }

    pub fn emit(&self, kind: &str, data: Value) -> Event {
        let ev = Event { seq: self.seq.fetch_add(1, Ordering::SeqCst) + 1, ts: now(), kind: kind.to_string(), data };
        {
            let mut buf = self.buf.lock().expect("event buffer");
            if buf.len() == BUFFER {
                buf.pop_front();
            }
            buf.push_back(ev.clone());
        }
        let _ = self.tx.send(ev.clone());
        ev
    }

    /// Sequence number of the newest event.
    pub fn last_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    /// Buffered events after `after_seq`, a live receiver, and the cursor
    /// actually used. A cursor beyond the newest event comes from a previous
    /// daemon instance (sequence numbers restart at boot), so the client
    /// gets this instance's full buffer, starting with its boot events.
    pub fn subscribe(&self, after_seq: u64) -> (Vec<Event>, broadcast::Receiver<Event>, u64) {
        let rx = self.tx.subscribe();
        let after = if after_seq > self.last_seq() { 0 } else { after_seq };
        let replay = self.buf.lock().expect("event buffer").iter().filter(|e| e.seq > after).cloned().collect();
        (replay, rx, after)
    }
}
