use std::time::Instant;

/// Frame counter and spinner cadence for the TUI's animations, advanced on a
/// fixed 100ms tick decoupled from the event-loop rate.
pub struct AnimationState {
    pub frame: usize,
    pub last_tick: Instant,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            frame: 0,
            last_tick: Instant::now(),
        }
    }
}

impl AnimationState {
    pub fn tick(&mut self) {
        if self.last_tick.elapsed() >= std::time::Duration::from_millis(100) {
            self.frame = self.frame.wrapping_add(1);
            self.last_tick = Instant::now();
        }
    }
}
