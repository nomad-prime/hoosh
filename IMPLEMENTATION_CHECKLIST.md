# Implementation Checklist

## Phase 1: Data Structures (Estimated: 1 hour)

### Step 1: BudgetInfo Struct
- [ ] Create `src/task_management/budget.rs` OR add to `mod.rs`
- [ ] Define struct with fields:
  - `start_time: Instant`
  - `timeout_seconds: u64`
  - `max_steps: usize`
  - `current_step: usize`
- [ ] Implement methods:
  - `new(timeout_seconds: u64, max_steps: usize) -> Self`
  - `elapsed_seconds(&self) -> u64`
  - `remaining_seconds(&self) -> u64`
  - `steps_remaining(&self) -> usize`
  - `progress_percentage(&self) -> f32`
  - `is_time_critical(&self) -> bool` (< 10s)
- [ ] Add unit tests for calculations
- [ ] **Commit**: "feat: Add BudgetInfo struct"

### Step 2: Constants & TaskDefinition Updates
- [ ] Add to `src/task_management/mod.rs`:
  ```rust
  pub const PLAN_AGENT_TIMEOUT_SECONDS: u64 = 600;
  pub const EXPLORE_AGENT_TIMEOUT_SECONDS: u64 = 300;
  ```
- [ ] Update `TaskDefinition::new()` to use constant instead of hardcoded 600
- [ ] **Commit**: "feat: Add timeout constants"

---

## Phase 2: System Message Enhancement (Estimated: 1 hour)

### Step 3: Update System Message Signature
- [ ] Modify `AgentType::system_message()` in `src/task_management/mod.rs`
- [ ] Change signature to:
  ```rust
  pub fn system_message(&self, task_prompt: &str, budget_info: Option<&BudgetInfo>) -> String
  ```
- [ ] **Commit**: "refactor: Update system_message signature"

### Step 4: Budget Guidance Formatter
- [ ] Add function to `src/task_management/mod.rs`:
  ```rust
  fn format_budget_guidance(budget_info: &BudgetInfo) -> String {
      format!("You have approximately {} seconds and {} steps remaining.", 
          budget_info.remaining_seconds(), 
          budget_info.steps_remaining())
  }
  ```
- [ ] **Commit**: "feat: Add budget guidance formatter"

### Step 5-6: Update Agent System Messages
- [ ] Update Plan agent message in `system_message()`
- [ ] Update Explore agent message in `system_message()`
- [ ] Add budget guidance to both templates
- [ ] Test that messages include budget when provided
- [ ] **Commit**: "feat: Include budget info in system messages"

---

## Phase 3: Agent Budget Tracking (Estimated: 1.5 hours)

### Step 7: Agent Struct Fields
- [ ] Open `src/agent/core.rs`
- [ ] Add to Agent struct:
  ```rust
  budget_info: Option<BudgetInfo>,
  start_time: Instant,
  budget_warning_sent: bool,
  ```
- [ ] Initialize in `Agent::new()`:
  ```rust
  start_time: Instant::now(),
  budget_info: None,
  budget_warning_sent: false,
  ```
- [ ] **Commit**: "feat: Add budget tracking fields to Agent"

### Step 8: Agent Builder Methods
- [ ] Add to Agent:
  ```rust
  pub fn with_budget_info(mut self, budget_info: BudgetInfo) -> Self {
      self.budget_info = Some(budget_info);
      self
  }
  ```
- [ ] Add getter methods:
  ```rust
  pub fn elapsed_seconds(&self) -> u64
  pub fn remaining_seconds(&self) -> u64
  pub fn steps_remaining(&self) -> usize
  ```
- [ ] **Commit**: "feat: Add budget info builder and getters"

### Step 9: Budget Check Method
- [ ] Add to Agent:
  ```rust
  fn check_resource_budget(&self, budget_info: &mut BudgetInfo) -> bool {
      budget_info.current_step = self.current_step; // Requires tracking
      
      if budget_info.remaining_seconds() < 10 
          || budget_info.steps_remaining() < 2 {
          return false;  // Budget exceeded
      }
      true  // Can continue
  }
  ```
- [ ] **Commit**: "feat: Add budget check method"

### Step 10: Graceful Conclusion Method
- [ ] Add to Agent:
  ```rust
  async fn generate_graceful_conclusion(
      &self, 
      conversation: &mut Conversation
  ) -> Result<()> {
      let prompt = "Time/steps budget exhausted. Provide final summary of work done so far.";
      // Request final response from LLM
      Ok(())
  }
  ```
- [ ] **Commit**: "feat: Add graceful conclusion method"

### Step 11: Update Handle Turn Loop
- [ ] Locate loop in `Agent::handle_turn()` at line ~124
- [ ] Add at start of loop:
  ```rust
  if let Some(mut info) = self.budget_info.clone() {
      if !self.check_resource_budget(&mut info) {
          self.send_event(AgentEvent::BudgetExceeded {
              reason: "TimeLimit".to_string(),
              elapsed_time: info.elapsed_seconds(),
              steps_taken: step,
          });
          self.generate_graceful_conclusion(conversation).await?;
          break;
      }
  }
  ```
- [ ] **Commit**: "feat: Add budget check to agent loop"

---

## Phase 4: Events (Estimated: 30 min)

### Step 12: Add New Event Variants
- [ ] Open `src/agent/agent_events.rs`
- [ ] Add variants:
  ```rust
  BudgetWarning {
      remaining_time: u64,
      remaining_steps: usize,
      time_percent: f32,
  },
  BudgetExceeded {
      reason: String,
      elapsed_time: u64,
      steps_taken: usize,
  },
  ```
- [ ] **Commit**: "feat: Add budget events"

### Step 13: Wire Up Event Sending
- [ ] In Agent loop, add budget warning check:
  ```rust
  if let Some(info) = &self.budget_info 
      && !self.budget_warning_sent
      && info.progress_percentage() < 30.0 {
      self.send_event(AgentEvent::BudgetWarning {
          remaining_time: info.remaining_seconds(),
          remaining_steps: info.steps_remaining(),
          time_percent: info.progress_percentage(),
      });
      self.budget_warning_sent = true;
  }
  ```
- [ ] **Commit**: "feat: Send budget warning events"

---

## Phase 5: TaskManager Integration (Estimated: 1 hour)

### Step 14: Create BudgetInfo in execute_task
- [ ] Open `src/task_management/task_manager.rs`
- [ ] In `execute_task()`, add after task_def received:
  ```rust
  let budget_info = BudgetInfo::new(
      task_def.timeout_seconds.unwrap_or(300),
      task_def.agent_type.max_steps(),
  );
  ```
- [ ] **Commit**: "feat: Create BudgetInfo in TaskManager"

### Step 15: Pass to Agent
- [ ] Update Agent creation:
  ```rust
  let agent = Agent::new(...)
      .with_max_steps(task_def.agent_type.max_steps())
      .with_budget_info(budget_info.clone())  // ADD THIS
      .with_event_sender(event_tx);
  ```
- [ ] **Commit**: "feat: Pass budget info to Agent"

### Step 16: Pass to System Message
- [ ] Update system message generation:
  ```rust
  let system_message = task_def.agent_type.system_message(
      &task_def.prompt,
      Some(&budget_info)  // ADD THIS PARAMETER
  );
  ```
- [ ] **Commit**: "feat: Pass budget info to system message"

---

## Phase 6: Tool Updates (Estimated: 1 hour)

### Step 17: Task Tool - Add Timeout Parameter
- [ ] Open `src/tools/task_tool.rs`
- [ ] Update TaskArgs struct:
  ```rust
  #[derive(Deserialize)]
  struct TaskArgs {
      subagent_type: String,
      prompt: String,
      description: String,
      #[serde(default)]
      model: Option<String>,
      #[serde(default)]
      timeout_seconds: Option<u64>,  // ADD THIS
  }
  ```
- [ ] **Commit**: "feat: Add timeout_seconds to TaskArgs"

### Step 18: Task Tool - Apply Timeout
- [ ] In `execute_impl()`:
  ```rust
  if let Some(timeout) = args.timeout_seconds {
      task_def = task_def.with_timeout(timeout);
  }
  ```
- [ ] **Commit**: "feat: Apply custom timeout in task tool"

### Step 19: Update Schema
- [ ] Update `parameter_schema()`:
  ```rust
  "timeout_seconds": {
      "type": "number",
      "description": "Optional timeout in seconds (default: 600 for plan, 300 for explore)",
      "minimum": 1,
      "maximum": 3600
  }
  ```
- [ ] **Commit**: "docs: Update task tool schema"

### Step 20: Bash Tool Configurability
- [ ] Open `src/tools/bash/tool.rs`
- [ ] Verify `with_timeout()` method exists
- [ ] If not, add:
  ```rust
  pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
      self.timeout_seconds = timeout_seconds;
      self
  }
  ```
- [ ] **Commit**: "feat: Ensure bash tool timeout is configurable"

---

## Phase 7: Testing (Estimated: 3-4 hours)

### Step 21: BudgetInfo Tests
- [ ] Create test file or add to `src/task_management/mod.rs` tests
- [ ] Test `elapsed_seconds()`:
  ```rust
  #[test]
  fn test_budget_info_elapsed_time() {
      let budget = BudgetInfo::new(600, 50);
      std::thread::sleep(Duration::from_millis(100));
      assert!(budget.elapsed_seconds() >= 0);
  }
  ```
- [ ] Test `remaining_seconds()`
- [ ] Test `is_time_critical()`
- [ ] **Commit**: "test: Add BudgetInfo unit tests"

### Step 22: Agent Budget Tests
- [ ] Add to `src/agent/core_tests.rs`
- [ ] Test Agent initializes with budget_info
- [ ] Test budget getters return correct values
- [ ] Test check_resource_budget() logic
- [ ] **Commit**: "test: Add Agent budget tracking tests"

### Step 23: Integration Test - Timeout
- [ ] Add to `src/task_management/task_manager.rs` tests
- [ ] Create task with very short timeout (2 seconds)
- [ ] Verify task fails with timeout message
- [ ] **Commit**: "test: Add integration test for task timeout"

### Step 24: Integration Test - Budget Warning
- [ ] Add to task manager tests
- [ ] Create task with 60-second timeout
- [ ] Verify BudgetWarning event sent
- [ ] **Commit**: "test: Add integration test for budget warning"

### Step 25: Update Existing Tests
- [ ] Fix test failures from Agent signature changes
- [ ] Update task manager tests
- [ ] Update task tool tests
- [ ] **Commit**: "test: Update existing tests for budget info"

### Step 26: Run Full Test Suite
- [ ] Execute `cargo test`
- [ ] Verify all tests pass
- [ ] Check for clippy warnings: `cargo clippy`
- [ ] Fix any warnings
- [ ] **Commit**: "test: All tests passing"

---

## Phase 8: Documentation & Review

### Step 27: Update Code Comments
- [ ] Add doc comments to BudgetInfo methods
- [ ] Document Agent budget fields
- [ ] Add inline comments to complex logic
- [ ] **Commit**: "docs: Add inline documentation"

### Step 28: Update README/Docs
- [ ] Document new timeout parameter in task tool
- [ ] Document budget awareness feature
- [ ] Add usage examples
- [ ] **Commit**: "docs: Update README with budget feature"

### Step 29: Final Review
- [ ] Code review (internal or peer)
- [ ] Verify no breaking changes
- [ ] Test with real subagent execution
- [ ] Check performance impact
- [ ] **Commit**: "refactor: Address review feedback"

---

## Final Checklist

General:
- [ ] All 26 steps completed
- [ ] No compiler errors
- [ ] No clippy warnings
- [ ] All tests passing (>90% coverage)

Code Quality:
- [ ] No hardcoded timeouts (use constants)
- [ ] Proper error handling
- [ ] Comments for complex logic
- [ ] No unwrap() without justification

Documentation:
- [ ] Inline comments added
- [ ] README updated
- [ ] Examples provided
- [ ] Architecture documented

Testing:
- [ ] Unit tests for BudgetInfo
- [ ] Unit tests for Agent budget logic
- [ ] Integration tests for timeout
- [ ] Integration tests for budget warnings
- [ ] Regression tests passing

---

## Git Commit Summary

```
feat: Add BudgetInfo struct for time/step tracking
feat: Add timeout constants for agent types
refactor: Update system_message signature
feat: Add budget guidance formatter
feat: Include budget info in system messages
feat: Add budget tracking fields to Agent
feat: Add budget info builder and getters
feat: Add budget check method
feat: Add graceful conclusion method
feat: Add budget check to agent loop
feat: Add budget events
feat: Send budget warning events
feat: Create BudgetInfo in TaskManager
feat: Pass budget info to Agent
feat: Pass budget info to system message
feat: Add timeout_seconds to TaskArgs
feat: Apply custom timeout in task tool
docs: Update task tool schema
feat: Ensure bash tool timeout is configurable
test: Add BudgetInfo unit tests
test: Add Agent budget tracking tests
test: Add integration test for task timeout
test: Add integration test for budget warning
test: Update existing tests for budget info
test: All tests passing
docs: Add inline documentation
docs: Update README with budget feature
refactor: Address review feedback
```

---

**Total Steps**: 29 (implemented as 45 micro-steps across 7 phases)  
**Estimated Time**: 3-4 days  
**Break Between Phases**: Recommended (commit and review each phase)
