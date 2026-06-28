/// Cumulative token counts and cost for the current conversation.
#[derive(Default)]
pub struct MetricsState {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_cost: f64,
}

impl MetricsState {
    pub fn record(&mut self, input_tokens: usize, output_tokens: usize, cost: Option<f64>) {
        self.input_tokens = input_tokens;
        self.output_tokens = output_tokens;
        if let Some(call_cost) = cost {
            self.total_cost += call_cost;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn has_usage(&self) -> bool {
        self.input_tokens > 0 || self.output_tokens > 0
    }
}
