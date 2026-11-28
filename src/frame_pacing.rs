//! Absolute frame deadlines avoid accumulating the oversleep of a relative FPS limiter.
use std::time::{Duration, Instant};

pub struct FramePacer {
    deadline: Instant,
    interval: Duration,
}
impl FramePacer {
    pub fn new(fps: u32) -> Self {
        let interval = Duration::from_secs_f64(1.0 / f64::from(fps.max(1)));
        Self {
            deadline: Instant::now() + interval,
            interval,
        }
    }
    /// Sleep for most of the idle interval. A bounded final spin reduces timer
    /// jitter; this is not a hard real-time guarantee against OS scheduling stalls.
    pub fn wait(&mut self) {
        if let Some(remaining) = self.deadline.checked_duration_since(Instant::now()) {
            let margin = Duration::from_millis(1);
            if remaining > margin {
                std::thread::sleep(remaining - margin);
            }
            while Instant::now() < self.deadline {
                std::hint::spin_loop();
            }
        }
        self.deadline += self.interval;
        if self.deadline < Instant::now() {
            self.deadline = Instant::now() + self.interval;
        }
    }
}
