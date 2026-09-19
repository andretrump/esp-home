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

    pub fn run(&mut self, f: impl FnOnce()) {
        if self.last.elapsed() >= self.interval {
            f();
            self.last = Instant::now();
        }
    }
}
