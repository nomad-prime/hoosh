use super::*;

#[test]
fn completion_state_new_initializes_correctly() {
    let state = CompletionState::new(0);
    assert_eq!(state.selected_index, 0);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.candidates.is_empty());
    assert!(state.query.is_empty());
    assert_eq!(state.completer_index, 0);
}

#[test]
fn completion_state_selected_item_returns_none_when_empty() {
    let state = CompletionState::new(0);
    assert_eq!(state.selected_item(), None);
}

#[test]
fn completion_state_selected_item_returns_correct_item() {
    let mut state = CompletionState::new(0);
    state.candidates = vec!["foo".to_string(), "bar".to_string()];
    assert_eq!(state.selected_item(), Some("foo"));

    state.selected_index = 1;
    assert_eq!(state.selected_item(), Some("bar"));
}

#[test]
fn completion_state_select_next_wraps_around() {
    let mut state = CompletionState::new(0);
    state.candidates = vec!["a".to_string(), "b".to_string(), "c".to_string()];

    state.select_next();
    assert_eq!(state.selected_index, 1);

    state.select_next();
    assert_eq!(state.selected_index, 2);

    state.select_next();
    assert_eq!(state.selected_index, 0); // wraps
}

#[test]
fn completion_state_select_prev_wraps_around() {
    let mut state = CompletionState::new(0);
    state.candidates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    state.selected_index = 0;

    state.select_prev();
    assert_eq!(state.selected_index, 2); // wraps to end

    state.select_prev();
    assert_eq!(state.selected_index, 1);
}

#[test]
fn completion_state_select_next_empty_candidates() {
    let mut state = CompletionState::new(0);
    state.select_next();
    assert_eq!(state.selected_index, 0);
}

#[test]
fn completion_state_scroll_offset_updates_when_scrolling() {
    let mut state = CompletionState::new(0);
    state.candidates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    state.selected_index = 2;

    state.update_scroll_offset(10);
    assert_eq!(state.scroll_offset, 0);

    // Test when selected_index would be out of view
    state.selected_index = 15;
    state.scroll_offset = 0;
    state.update_scroll_offset(10);
    assert_eq!(state.scroll_offset, 6); // 15 - (10 - 1) = 6
}

#[test]
fn approval_dialog_new_initializes_correctly() {
    let dialog = ApprovalDialogState::new("call123".to_string(), "bash".to_string());
    assert_eq!(dialog.tool_call_id, "call123");
    assert_eq!(dialog.tool_name, "bash");
    assert_eq!(dialog.selected_index, 0);
}

#[test]
fn active_tool_call_add_subagent_step() {
    let mut tool_call = ActiveToolCall {
        tool_call_id: "call1".to_string(),
        display_name: "test".to_string(),
        status: ToolCallStatus::Starting,
        preview: None,
        budget_pct: None,
        result_summary: None,
        subagent_steps: Vec::new(),
        is_subagent_task: false,
        bash_output_lines: Vec::new(),
        is_bash_streaming: false,
        start_time: Instant::now(),
    };

    let step = SubagentStepSummary {
        step_number: 1,
        action_type: "search".to_string(),
        description: "searching for data".to_string(),
    };

    tool_call.add_subagent_step(step);
    assert_eq!(tool_call.subagent_steps.len(), 1);
    assert_eq!(tool_call.subagent_steps[0].step_number, 1);
}

#[test]
fn active_tool_call_add_bash_output_line() {
    let mut tool_call = ActiveToolCall {
        tool_call_id: "call1".to_string(),
        display_name: "bash".to_string(),
        status: ToolCallStatus::Executing,
        preview: None,
        result_summary: None,
        subagent_steps: Vec::new(),
        is_subagent_task: false,
        bash_output_lines: Vec::new(),
        is_bash_streaming: false,
        budget_pct: None,
        start_time: Instant::now(),
    };

    let line = BashOutputLine {
        line_number: 1,
        content: "Hello from bash".to_string(),
        stream_type: "stdout".to_string(),
    };

    tool_call.add_bash_output_line(line);
    assert_eq!(tool_call.bash_output_lines.len(), 1);
    assert_eq!(tool_call.bash_output_lines[0].line_number, 1);
    assert_eq!(tool_call.bash_output_lines[0].content, "Hello from bash");
    assert_eq!(tool_call.bash_output_lines[0].stream_type, "stdout");
    assert!(tool_call.is_bash_streaming);
}

// ============================================================================
// AppState Tests - Initialization
// ============================================================================

#[test]
fn app_state_new_initializes_correctly() {
    let state = AppState::new();
    assert_eq!(state.agent_state, AgentState::Idle);
    assert!(!state.should_quit);
    assert!(!state.should_cancel_task);
    assert_eq!(state.max_messages, 1000);
    assert!(state.messages.is_empty());
    assert!(state.completion_state.is_none());
    assert!(state.approval_dialog_state.is_none());
    assert!(state.tool_permission_dialog_state.is_none());
    assert!(
        !state
            .autopilot_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    assert_eq!(state.input_tokens, 0);
    assert_eq!(state.output_tokens, 0);
    assert_eq!(state.total_cost, 0.0);
}

#[test]
fn app_state_default_creates_new() {
    let state = AppState::default();
    assert_eq!(state.agent_state, AgentState::Idle);
}

#[test]
fn app_state_tick_animation_increments() {
    let mut state = AppState::new();
    let initial = state.animation_frame;
    state.tick_animation();
    assert_eq!(state.animation_frame, initial.wrapping_add(1));
}

#[test]
fn app_state_toggle_autopilot() {
    let mut state = AppState::new();
    assert!(
        !state
            .autopilot_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    state.toggle_autopilot();
    assert!(
        state
            .autopilot_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    state.toggle_autopilot();
    assert!(
        !state
            .autopilot_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[test]
fn app_state_is_completing_returns_correct_state() {
    let mut state = AppState::new();
    assert!(!state.is_completing());

    state.start_completion(0);
    assert!(state.is_completing());

    state.cancel_completion();
    assert!(!state.is_completing());
}

#[test]
fn app_state_start_completion_creates_state() {
    let mut state = AppState::new();
    state.start_completion(1);

    assert!(state.completion_state.is_some());
    let comp = state.completion_state.as_ref().unwrap();
    assert_eq!(comp.completer_index, 1);
}

#[test]
fn app_state_update_completion_query() {
    let mut state = AppState::new();
    state.start_completion(0);
    state.update_completion_query("test".to_string());

    let comp = state.completion_state.as_ref().unwrap();
    assert_eq!(comp.query, "test");
    assert_eq!(comp.selected_index, 0);
}

#[test]
fn app_state_set_completion_candidates() {
    let mut state = AppState::new();
    state.start_completion(0);

    let candidates = vec!["foo".to_string(), "bar".to_string()];
    state.set_completion_candidates(candidates);

    let comp = state.completion_state.as_ref().unwrap();
    assert_eq!(comp.candidates.len(), 2);
    assert_eq!(comp.selected_index, 0);
}

#[test]
fn app_state_select_next_completion() {
    let mut state = AppState::new();
    state.start_completion(0);
    state.set_completion_candidates(vec!["a".to_string(), "b".to_string()]);

    state.select_next_completion();
    assert_eq!(state.completion_state.as_ref().unwrap().selected_index, 1);
}

#[test]
fn app_state_apply_completion_returns_selected() {
    let mut state = AppState::new();
    state.start_completion(0);
    state.set_completion_candidates(vec!["test".to_string()]);

    let result = state.apply_completion();
    assert_eq!(result, Some("test".to_string()));
    assert!(!state.is_completing());
}

#[test]
fn app_state_apply_completion_without_state() {
    let mut state = AppState::new();
    let result = state.apply_completion();
    assert_eq!(result, None);
}

#[test]
fn app_state_show_approval_dialog() {
    let mut state = AppState::new();
    state.show_approval_dialog("call1".to_string(), "bash".to_string());

    assert!(state.is_showing_approval_dialog());
    let dialog = state.approval_dialog_state.as_ref().unwrap();
    assert_eq!(dialog.tool_call_id, "call1");
    assert_eq!(dialog.tool_name, "bash");
}

#[test]
fn app_state_hide_approval_dialog() {
    let mut state = AppState::new();
    state.show_approval_dialog("call1".to_string(), "bash".to_string());
    assert!(state.is_showing_approval_dialog());

    state.hide_approval_dialog();
    assert!(!state.is_showing_approval_dialog());
}

#[test]
fn app_state_select_approval_options() {
    let mut state = AppState::new();
    state.show_approval_dialog("call1".to_string(), "bash".to_string());

    state.select_next_approval_option();
    assert_eq!(
        state.approval_dialog_state.as_ref().unwrap().selected_index,
        1
    );

    state.select_next_approval_option();
    assert_eq!(
        state.approval_dialog_state.as_ref().unwrap().selected_index,
        0
    );
}

#[test]
fn app_state_add_message() {
    let mut state = AppState::new();
    state.add_message("Hello".to_string());

    assert_eq!(state.messages.len(), 1);
    assert!(state.has_pending_messages());
}

#[test]
fn app_state_messages_respects_max_size() {
    let mut state = AppState::new();
    state.max_messages = 3;

    state.add_message("msg1".to_string());
    state.add_message("msg2".to_string());
    state.add_message("msg3".to_string());
    state.add_message("msg4".to_string());

    assert_eq!(state.messages.len(), 3);
}

#[test]
fn app_state_add_debug_message() {
    let mut state = AppState::new();
    state.add_debug_message("debug info".to_string());

    assert_eq!(state.messages.len(), 1);
    assert!(state.has_pending_messages());
}

#[test]
fn app_state_has_pending_messages() {
    let mut state = AppState::new();
    assert!(!state.has_pending_messages());

    state.add_message("test".to_string());
    assert!(state.has_pending_messages());
}

#[test]
fn app_state_drain_pending_messages() {
    let mut state = AppState::new();
    state.add_message("msg1".to_string());
    state.add_message("msg2".to_string());

    let messages = state.drain_pending_messages();
    assert_eq!(messages.len(), 2);
    assert!(!state.has_pending_messages());
}

#[test]
fn app_state_get_input_text() {
    let mut state = AppState::new();
    state.input.insert_str("hello\nworld");

    let text = state.get_input_text();
    assert!(text.contains("hello"));
    assert!(text.contains("world"));
}

#[test]
fn app_state_clear_input() {
    let mut state = AppState::new();
    state.input.insert_str("hello");
    assert!(!state.get_input_text().is_empty());

    state.clear_input();
    assert!(state.get_input_text().is_empty());
}

#[test]
fn app_state_add_active_tool_call() {
    let mut state = AppState::new();
    state.add_active_tool_call("call1".to_string(), "bash".to_string());

    assert_eq!(state.active_tool_calls.len(), 1);
    let tool = &state.active_tool_calls[0];
    assert_eq!(tool.tool_call_id, "call1");
    assert_eq!(tool.display_name, "bash");
    assert_eq!(tool.status, ToolCallStatus::Starting);
}

#[test]
fn app_state_update_tool_call_status() {
    let mut state = AppState::new();
    state.add_active_tool_call("call1".to_string(), "bash".to_string());

    state.update_tool_call_status("call1", ToolCallStatus::Executing);
    assert_eq!(state.active_tool_calls[0].status, ToolCallStatus::Executing);
}

#[test]
fn app_state_set_tool_call_result() {
    let mut state = AppState::new();
    state.add_active_tool_call("call1".to_string(), "bash".to_string());

    state.set_tool_call_result("call1", "success".to_string());
    assert_eq!(
        state.active_tool_calls[0].result_summary,
        Some("success".to_string())
    );
}

#[test]
fn app_state_get_active_tool_call_mut() {
    let mut state = AppState::new();
    state.add_active_tool_call("call1".to_string(), "bash".to_string());

    let tool = state.get_active_tool_call_mut("call1");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().display_name, "bash");
}

#[test]
fn app_state_complete_single_tool_call() {
    let mut state = AppState::new();
    state.add_active_tool_call("call1".to_string(), "bash".to_string());
    state.set_tool_call_result("call1", "result".to_string());

    state.complete_single_tool_call("call1");
    assert!(state.active_tool_calls.is_empty());
    assert!(state.has_pending_messages());
}

#[test]
fn app_state_clear_active_tool_calls() {
    let mut state = AppState::new();
    state.add_active_tool_call("call1".to_string(), "bash".to_string());
    state.add_active_tool_call("call2".to_string(), "python".to_string());

    state.clear_active_tool_calls();
    assert!(state.active_tool_calls.is_empty());
}

#[test]
fn app_state_add_thought() {
    let mut state = AppState::new();
    state.add_thought("thinking...");

    // add_thought adds a newline message first, then the content
    assert_eq!(state.messages.len(), 2);
}

#[test]
fn app_state_add_thought_empty_skips() {
    let mut state = AppState::new();
    state.add_thought("");

    assert_eq!(state.messages.len(), 0);
}

#[test]
fn app_state_add_tool_call_message() {
    let mut state = AppState::new();
    state.add_tool_call("bash");

    assert_eq!(state.messages.len(), 1);
}

#[test]
fn app_state_add_status_message() {
    let mut state = AppState::new();
    state.add_status_message("status");

    assert_eq!(state.messages.len(), 1);
}

#[test]
fn app_state_add_error() {
    let mut state = AppState::new();
    state.add_error("error message");

    assert_eq!(state.messages.len(), 1);
}

#[test]
fn app_state_add_final_response() {
    let mut state = AppState::new();
    state.add_final_response("response content");

    assert!(!state.messages.is_empty());
}

#[test]
fn app_state_add_user_input() {
    let mut state = AppState::new();
    state.add_user_input("user query");

    assert_eq!(state.messages.len(), 1);
}

#[test]
fn app_state_add_retry_failure() {
    let mut state = AppState::new();
    state.add_retry_failure("retry failed");

    assert_eq!(state.messages.len(), 1);
}
