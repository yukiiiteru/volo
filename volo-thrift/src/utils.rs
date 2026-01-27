use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

pub struct DebugCounter {
    prefix: &'static str,
    active: AtomicUsize,
    open: AtomicUsize,
    close: AtomicUsize,
    sleep: Duration,
}

impl DebugCounter {
    pub fn new(prefix: &'static str, sleep_ms: u64) -> Arc<Self> {
        let this = Self {
            prefix,
            active: AtomicUsize::new(0),
            open: AtomicUsize::new(0),
            close: AtomicUsize::new(0),
            sleep: Duration::from_millis(sleep_ms),
        };
        let this = Arc::new(this);

        tokio::spawn(this.clone().print_worker());

        this
    }

    async fn print_worker(self: Arc<Self>) {
        let prefix = self.prefix;
        loop {
            let time = time();
            let active = self.active.load(Ordering::Relaxed);
            let open = self.open.swap(0, Ordering::Relaxed);
            let close = self.close.swap(0, Ordering::Relaxed);
            let diff = (open as isize) - (close as isize);
            tracing::info!(
                "{prefix} {time} diff: {diff} active: {active} open: {open} close: {close}"
            );
            tokio::time::sleep(self.sleep).await;
        }
    }

    pub fn inc(&self) {
        self.active.fetch_add(1, Ordering::Relaxed);
        self.open.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec(&self) {
        self.active.fetch_sub(1, Ordering::Relaxed);
        self.close.fetch_add(1, Ordering::Relaxed);
    }
}

fn time() -> String {
    chrono::Local::now().format("%%H:%M:%S").to_string()
}
