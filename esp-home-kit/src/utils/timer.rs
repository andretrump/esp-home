use std::time::{Duration, Instant};

pub struct Timer {
    interval: Duration,
    last: Instant,
}

impl Timer {
    pub fn new(secs: u64) -> Self {
        Self {
            interval: Duration::from_secs(secs),
            last: Instant::now(),
        }
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.last.elapsed().as_secs()
    }

    pub fn has_elapsed(&self) -> bool {
        self.last.elapsed() >= self.interval
    }

    pub fn reset(&mut self) {
        self.last = Instant::now();
    }

    pub fn run(&mut self, f: impl FnOnce()) {
        if self.has_elapsed() {
            f();
            self.reset();
        }
    }
}
