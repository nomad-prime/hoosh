Refactor src/conversations/handler.rs
Complexity: 10
Effort: 2-3 hours
Impact: Medium - core business logic
Problems:

handle_tool_calls() has nested async operations
Event emission scattered throughout
Inconsistent error handling between user rejection and other errors

Refactoring Actions:
rust// Extract event emission to trait
trait EventEmitter {
fn emit(&self, event: AgentEvent);
}

impl ConversationHandler {
async fn handle_tool_calls(&self, ...) -> Result<TurnStatus> {
// Phase 1: Emit tool call events
self.emit_tool_call_events(&tool_calls);

        // Phase 2: Execute tools
        let results = self.execute_tools(&tool_calls).await?;
        
        // Phase 3: Check for rejections
        if self.has_user_rejection(&results) {
            return Ok(TurnStatus::Complete);
        }
        
        // Phase 4: Emit results and update conversation
        self.emit_tool_results(&results);
        conversation.add_tool_results(results);
        
        Ok(TurnStatus::Continue)
    }
    
    fn emit_tool_call_events(&self, tool_calls: &[ToolCall]) { ... }
    async fn execute_tools(&self, tool_calls: &[ToolCall]) -> Result<Vec<ToolResult>> { ... }
    fn has_user_rejection(&self, results: &[ToolResult]) -> bool { ... }
    fn emit_tool_results(&self, results: &[ToolResult]) { ... }

}
Specific Steps:

Extract event emission to helper methods
Split tool execution into separate function
Extract rejection checking logic
Use Result type consistently

Success Metrics:

handle_tool_calls() <40 lines
Each phase clearly separated
Complexity drops from 10 to <6
