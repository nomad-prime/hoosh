use serde::Serialize;
use std::cmp::min;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct BudgetInfo {
    pub elapsed_seconds: u64,
    pub remaining_seconds: u64,
    pub total_steps: usize,
    pub max_steps: usize,
}

#[derive(Debug, Clone)]
pub struct ExecutionBudget {
    pub start_time: Instant,
    pub max_duration: Duration,
    pub max_steps: usize,
    pause_start: Option<Instant>,
    total_paused_duration: Duration,
}

impl ExecutionBudget {
    pub fn new(max_duration: Duration, max_steps: usize) -> Self {
        Self {
            start_time: Instant::now(),
            max_duration,
            max_steps,
            pause_start: None,
            total_paused_duration: Duration::ZERO,
        }
    }

    pub fn pause(&mut self) {
        self.pause_start = Some(Instant::now());
    }

    pub fn resume(&mut self) {
        if let Some(start) = self.pause_start {
            self.total_paused_duration += start.elapsed();
            self.pause_start = None;
        }
    }

    pub fn elapsed_seconds(&self) -> u64 {
        let raw_elapsed = self.start_time.elapsed().as_secs();
        raw_elapsed.saturating_sub(self.total_paused_duration.as_secs())
    }

    pub fn remaining_seconds(&self) -> u64 {
        let elapsed = self.elapsed_seconds();
        if elapsed >= self.max_duration.as_secs() {
            0
        } else {
            self.max_duration.as_secs() - elapsed
        }
    }

    pub fn time_percentage_used(&self) -> f32 {
        let total = self.max_duration.as_secs();
        if total == 0 {
            0.0
        } else {
            (min(self.elapsed_seconds(), total) as f32 / total as f32) * 100.0
        }
    }

    pub fn steps_percentage_used(&self, current_step: usize) -> f32 {
        if self.max_steps == 0 {
            0.0
        } else {
            (min(current_step, self.max_steps) as f32 / self.max_steps as f32) * 100.0
        }
    }

    pub fn percentage_used(&self, current_step: usize) -> f32 {
        self.steps_percentage_used(current_step)
            .max(self.time_percentage_used())
    }

    pub fn should_wrap_up(&self, current_step: usize) -> bool {
        let time_pressure = self.time_percentage_used();
        let step_pressure = self.steps_percentage_used(current_step);
        let max_pressure = time_pressure.max(step_pressure);

        // Threshold for wrapping up: 70% of either budget used.
        let low_budget_threshold = 70.0;

        max_pressure >= low_budget_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_execution_budget_elapsed_seconds() {
        let budget = ExecutionBudget::new(Duration::from_secs(60), 10);
        std::thread::sleep(Duration::from_millis(100));
        assert!(budget.elapsed_seconds() as i64 <= 1);
    }

    #[test]
    fn test_execution_budget_remaining_seconds() {
        let budget = ExecutionBudget::new(Duration::from_secs(60), 10);
        std::thread::sleep(Duration::from_millis(5));
        assert!((budget.remaining_seconds() - 59) as i64 <= 1);

        let budget_short = ExecutionBudget::new(Duration::from_millis(50), 10);
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(budget_short.remaining_seconds(), 0);
    }

    #[test]
    fn test_execution_budget_time_percentage_used() {
        let budget = ExecutionBudget::new(Duration::from_secs(10), 10);
        let initial_percentage = budget.time_percentage_used();
        assert!((0.0..10.0).contains(&initial_percentage));

        let budget_zero = ExecutionBudget::new(Duration::from_secs(0), 10);
        assert_eq!(budget_zero.time_percentage_used(), 0.0);
    }

    #[test]
    fn test_execution_budget_steps_percentage_used() {
        let budget = ExecutionBudget::new(Duration::from_secs(100), 10);
        assert_eq!(budget.steps_percentage_used(5), 50.0);
        assert_eq!(budget.steps_percentage_used(10), 100.0);
        assert_eq!(budget.steps_percentage_used(15), 100.0);
        assert_eq!(budget.steps_percentage_used(0), 0.0);
    }

    #[test]
    fn test_execution_budget_should_wrap_up() {
        // Test step-based pressure (75% steps -> should wrap up)
        let budget_steps = ExecutionBudget::new(Duration::from_secs(100), 4);
        assert!(budget_steps.should_wrap_up(3));

        // Not enough pressure (40% steps)
        let budget_low = ExecutionBudget::new(Duration::from_secs(100), 50);
        assert!(!budget_low.should_wrap_up(20));

        // Time-based test - use a short duration to ensure it triggers
        // Note: Using seconds instead of milliseconds because elapsed_seconds() uses as_secs()
        // We need to sleep for >= 1 second for elapsed_seconds() to return 1
        let budget_time = ExecutionBudget::new(Duration::from_secs(1), 100);
        std::thread::sleep(Duration::from_millis(1100));
        assert!(budget_time.should_wrap_up(5));
    }
}

