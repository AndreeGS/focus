use std::time::{Duration, Instant};

/// Uses elapsed monotonic time, never the number of UI updates.
#[derive(Debug)]
pub struct Timer {
    duration: Duration,
    elapsed: Duration,
    running_since: Option<Instant>,
    active: bool,
}

impl Timer {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            elapsed: Duration::ZERO,
            running_since: None,
            active: false,
        }
    }

    pub fn start(&mut self, now: Instant) {
        if self.running_since.is_none() {
            self.active = true;
            self.running_since = Some(now);
        }
    }

    pub fn pause(&mut self, now: Instant) {
        if let Some(since) = self.running_since.take() {
            self.elapsed += now.saturating_duration_since(since);
        }
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        let elapsed = self.elapsed
            + self
                .running_since
                .map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        self.duration.saturating_sub(elapsed)
    }

    pub fn overdue(&self, now: Instant) -> Duration {
        let elapsed = self.elapsed
            + self
                .running_since
                .map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        elapsed.saturating_sub(self.duration)
    }

    /// A completion is emitted exactly once, even after a delayed frame.
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.running_since.is_some() && self.remaining(now).is_zero() {
            self.running_since = None;
            self.elapsed = self.duration;
            self.active = false;
            return true;
        }
        false
    }

    pub fn active(&self) -> bool {
        self.active
    }
    pub fn running(&self) -> bool {
        self.running_since.is_some()
    }
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_frame_finishes_only_once() {
        let now = Instant::now();
        let mut timer = Timer::new(Duration::from_secs(60));
        timer.start(now);
        assert!(!timer.tick(now + Duration::from_secs(59)));
        assert!(timer.tick(now + Duration::from_secs(90)));
        assert!(!timer.tick(now + Duration::from_secs(100)));
        assert!(!timer.active());
    }

    #[test]
    fn paused_time_does_not_count() {
        let now = Instant::now();
        let mut timer = Timer::new(Duration::from_secs(60));
        timer.start(now);
        timer.pause(now + Duration::from_secs(20));
        assert_eq!(
            timer.remaining(now + Duration::from_secs(200)),
            Duration::from_secs(40)
        );
        assert!(!timer.tick(now + Duration::from_secs(200)));
        assert!(timer.active());
        timer.start(now + Duration::from_secs(200));
        assert!(!timer.tick(now + Duration::from_secs(239)));
        assert!(timer.tick(now + Duration::from_secs(240)));
    }

    #[test]
    fn repeated_start_does_not_extend_deadline() {
        let now = Instant::now();
        let mut timer = Timer::new(Duration::from_secs(10));
        timer.start(now);
        timer.start(now + Duration::from_secs(9));
        assert!(timer.tick(now + Duration::from_secs(10)));
    }

    #[test]
    fn delayed_completion_preserves_deadline_offset() {
        let now = Instant::now();
        let mut timer = Timer::new(Duration::from_secs(60));
        timer.start(now);
        assert_eq!(
            timer.overdue(now + Duration::from_secs(75)),
            Duration::from_secs(15)
        );
        assert_eq!(
            timer.remaining(now + Duration::from_secs(75)),
            Duration::ZERO
        );
    }
}
