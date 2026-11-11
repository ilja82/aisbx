//! Ends the dashboard when its terminal is gone and reading input never returns.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// How long one input read may take before the terminal counts as gone.
pub const LIMIT: Duration = Duration::from_secs(5);

/// How often the watchdog looks.
const EVERY: Duration = Duration::from_millis(500);

/// Exit code when the watchdog ends the dashboard: 128 + SIGHUP, as for a hangup.
pub const EXIT_CODE: i32 = 129;

/// Tracks whether the event loop is inside an input read, and since when.
pub struct InputWatch {
    start: Instant,
    /// Milliseconds since `start`, plus one, at which the current read began; 0 outside a read.
    entered: AtomicU64,
}

impl InputWatch {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            start: Instant::now(),
            entered: AtomicU64::new(0),
        })
    }

    fn now(&self) -> u64 {
        self.start.elapsed().as_millis() as u64 + 1
    }

    /// Run one input read under the watch.
    pub fn read<T>(&self, f: impl FnOnce() -> T) -> T {
        self.entered.store(self.now(), Ordering::Relaxed);
        let result = f();
        self.entered.store(0, Ordering::Relaxed);
        result
    }

    /// How long the current read has run, or `None` outside a read.
    pub fn reading_for(&self) -> Option<Duration> {
        match self.entered.load(Ordering::Relaxed) {
            0 => None,
            at => Some(Duration::from_millis(self.now().saturating_sub(at))),
        }
    }
}

/// Start the watchdog thread: `on_stuck` runs once a single read has taken `limit`.
pub fn spawn(watch: Arc<InputWatch>, limit: Duration, on_stuck: impl FnOnce() + Send + 'static) {
    spawn_every(watch, limit, EVERY, on_stuck);
}

fn spawn_every(
    watch: Arc<InputWatch>,
    limit: Duration,
    every: Duration,
    on_stuck: impl FnOnce() + Send + 'static,
) {
    let _ = thread::Builder::new()
        .name("input-watchdog".into())
        .spawn(move || loop {
            thread::sleep(every);
            if watch.reading_for().is_some_and(|d| d >= limit) {
                on_stuck();
                return;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    const SHORT: Duration = Duration::from_millis(150);
    const TICK: Duration = Duration::from_millis(10);

    #[test]
    fn a_read_counts_only_while_it_runs() {
        let watch = InputWatch::new();
        assert_eq!(watch.reading_for(), None);
        let inside = watch.read(|| {
            thread::sleep(Duration::from_millis(30));
            watch.reading_for()
        });
        assert!(inside.is_some_and(|d| d >= Duration::from_millis(30)));
        assert_eq!(watch.reading_for(), None);
    }

    #[test]
    fn a_read_that_never_returns_ends_the_dashboard() {
        let watch = InputWatch::new();
        let (tx, rx) = mpsc::channel();
        spawn_every(Arc::clone(&watch), SHORT, TICK, move || {
            let _ = tx.send(());
        });
        watch.read(|| thread::sleep(SHORT * 4));
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn short_reads_never_end_it() {
        let watch = InputWatch::new();
        let (tx, rx) = mpsc::channel();
        spawn_every(Arc::clone(&watch), SHORT, TICK, move || {
            let _ = tx.send(());
        });
        for _ in 0..30 {
            watch.read(|| thread::sleep(Duration::from_millis(20)));
            thread::sleep(Duration::from_millis(5));
        }
        assert!(rx.try_recv().is_err());
    }
}
