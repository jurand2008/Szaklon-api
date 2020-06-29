use std::time::Instant;

pub struct PerfLog {
    start: Instant,
}

impl PerfLog {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn log(self, name: &str) {
        let diff = self.start.elapsed();
        log::debug!("perf: {} {}ms", name, diff.as_micros() as f64 / 1_000.0);
    }
}
