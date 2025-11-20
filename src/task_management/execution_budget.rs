use serde::Serialize;
use std::cmp::min;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct BudgetInfo {
    pub elapsed_seconds: u64,
    pub remaining_seconds: u64,
    pub steps_completed: usize,
    pub max_steps: usize,
}

#[derive(Debug, Clone)]
pub struct ExecutionBudget {
    pub start_time: Instant,
    pub max_duration: Duration,
    pub max_steps: usize,
}

impl ExecutionBudget {
    pub fn new(max_duration: Duration, max_steps: usize) -> Self {
        Self {
            start_time: Instant::now(),
            max_duration,
            max_steps,
        }
    }

    pub fn elapsed_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
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

    pub fn should_wrap_up(&self, current_step: usize) -> bool {
        let time_pressure = self.time_percentage_used();
        let step_pressure = self.steps_percentage_used(current_step);
        let max_pressure = time_pressure.max(step_pressure);

        // Threshold for wrapping up: 70% of either budget used.
        // Also consider a hard limit of 80% if still running past the soft wrap-up.
        let low_budget_threshold = 70.0;
        let hard_exit_threshold = 80.0; // This might be used by the caller to force an exit

        max_pressure >= low_budget_threshold ||
            self.elapsed_seconds() >= (self.max_duration.as_secs_f32() * hard_exit_threshold / 100.0).floor() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_execution_budget_elapsed_seconds() {
        let budget = ExecutionBudget::new(Duration::from_secs(60), 10);
        std::thread::sleep(Duration::from_millis(100));
        assert!((budget.elapsed_seconds() - 0) as i64 <= 1);
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
        let budget = ExecutionBudget::new(Duration::from_secs(1), 10);
        std::thread::sleep(Duration::from_millis(200));
        assert!((budget.time_percentage_used() - 20.0).abs() < 1.0);

        let budget_over = ExecutionBudget::new(Duration::from_millis(100), 10);
        std::thread::sleep(Duration::from_millis(150));
        assert!((budget_over.time_percentage_used() - 100.0).abs() < 1.0);
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
        // Low budget (70% time, 50% steps -> max 70% pressure)
        let budget = ExecutionBudget::new(Duration::from_millis(100), 10);
        std::thread::sleep(Duration::from_millis(70));
        assert!(budget.should_wrap_up(5));

        // High steps usage (50% time, 75% steps -> max 75% pressure)
        let budget_steps = ExecutionBudget::new(Duration::from_millis(100), 4);
        std::thread::sleep(Duration::from_millis(50));
        assert!(budget_steps.should_wrap_up(3));

        // Not enough pressure
        let budget_low = ExecutionBudget::new(Duration::from_millis(100), 50);
        std::thread::sleep(Duration::from_millis(50));
        assert!(!budget_low.should_wrap_up(20));

        // Hard exit threshold (80% time used)
        let budget_hard_time = ExecutionBudget::new(Duration::from_millis(100), 100);
        std::thread::sleep(Duration::from_millis(80));
        assert!(budget_hard_time.should_wrap_up(5));

        // Not enough time elapsed for hard exit
        let budget_not_hard_time = ExecutionBudget::new(Duration::from_millis(100), 100);
        std::thread::sleep(Duration::from_millis(70));
        assert!(!budget_not_hard_time.should_wrap_up(8));
    }
}
