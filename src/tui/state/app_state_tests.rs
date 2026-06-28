use super::*;
use crate::tools::phrasing;

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
        render: ToolRender::Standard,
        phrasing: phrasing::GENERIC,
        status: ToolCallStatus::Starting,
        preview: None,
        budget_pct: None,
        result_summary: None,
        subagent_steps: Vec::new(),
        is_subagent_task: false,
        bash_output_lines: Vec::new(),
        is_bash_streaming: false,
        start_time: Instant::now(),
        total_tool_uses: None,
        total_tokens: None,
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
        render: ToolRender::Standard,
        phrasing: phrasing::GENERIC,
        status: ToolCallStatus::Executing,
        preview: None,
        result_summary: None,
        subagent_steps: Vec::new(),
        is_subagent_task: false,
        bash_output_lines: Vec::new(),
        is_bash_streaming: false,
        budget_pct: None,
        start_time: Instant::now(),
        total_tool_uses: None,
        total_tokens: None,
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
    assert_eq!(state.max_messages, 100_000);
    assert!(state.messages.is_empty());
    assert!(state.completion_state.is_none());
    assert!(state.dialogs.approval.is_none());
    assert!(state.dialogs.permission.is_none());
    assert!(
        !state
            .autopilot_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    assert_eq!(state.metrics.input_tokens, 0);
    assert_eq!(state.metrics.output_tokens, 0);
    assert_eq!(state.metrics.total_cost, 0.0);
}

#[test]
fn app_state_default_creates_new() {
    let state = AppState::default();
    assert_eq!(state.agent_state, AgentState::Idle);
}

#[test]
fn app_state_new_has_no_cancelled_prompt_and_quit_disarmed() {
    let state = AppState::new();
    assert!(state.last_submitted_input.is_none());
    assert!(!state.quit_armed);
}

#[test]
fn app_state_new_has_empty_queue() {
    let state = AppState::new();
    assert!(state.queued_prompts.is_empty());
}

#[test]
fn queue_push_and_pop_preserves_order() {
    let mut state = AppState::new();
    state.queued_prompts.push_back("first".into());
    state.queued_prompts.push_back("second".into());
    state.queued_prompts.push_back("third".into());
    assert_eq!(state.queued_prompts.pop_front().as_deref(), Some("first"));
    assert_eq!(state.queued_prompts.pop_front().as_deref(), Some("second"));
    assert_eq!(state.queued_prompts.pop_front().as_deref(), Some("third"));
    assert!(state.queued_prompts.is_empty());
}

#[test]
fn set_input_text_replaces_buffer_and_preserves_newlines() {
    let mut state = AppState::new();
    state.set_input_text("hello\nworld");
    assert_eq!(state.get_input_text(), "hello\nworld");

    state.set_input_text("replaced");
    assert_eq!(state.get_input_text(), "replaced");
}

#[test]
fn set_input_text_with_empty_clears_input() {
    let mut state = AppState::new();
    state.set_input_text("something");
    state.set_input_text("");
    assert_eq!(state.get_input_text(), "");
}

#[test]
fn text_deltas_accumulate_into_streaming_buffer() {
    let mut state = AppState::new();
    state.handle_agent_event(AgentEvent::StreamStarted);
    state.handle_agent_event(AgentEvent::TextDelta("Hello ".into()));
    state.handle_agent_event(AgentEvent::TextDelta("world".into()));
    assert_eq!(state.streaming.text.as_deref(), Some("Hello world"));
}

#[test]
fn visible_streaming_text_shows_partial_trailing_line() {
    let mut state = AppState::new();
    state.handle_agent_event(AgentEvent::StreamStarted);
    state.handle_agent_event(AgentEvent::TextDelta("line one\npartial".into()));
    assert_eq!(state.visible_streaming_text(), Some("line one\npartial"));
}

#[test]
fn visible_streaming_text_shows_text_before_first_newline() {
    let mut state = AppState::new();
    state.handle_agent_event(AgentEvent::StreamStarted);
    state.handle_agent_event(AgentEvent::TextDelta("no newline yet".into()));
    assert_eq!(state.visible_streaming_text(), Some("no newline yet"));
}

#[test]
fn visible_streaming_text_is_none_when_empty() {
    let mut state = AppState::new();
    state.handle_agent_event(AgentEvent::StreamStarted);
    assert_eq!(state.visible_streaming_text(), None);
}

#[test]
fn final_response_clears_streaming_buffer() {
    let mut state = AppState::new();
    state.handle_agent_event(AgentEvent::StreamStarted);
    state.handle_agent_event(AgentEvent::TextDelta("partial".into()));
    state.handle_agent_event(AgentEvent::FinalResponse("partial answer".into()));
    assert!(state.streaming.text.is_none());
}

#[test]
fn app_state_tick_animation_increments_after_interval() {
    let mut state = AppState::new();
    let initial = state.animation.frame;

    state.tick_animation();
    assert_eq!(
        state.animation.frame, initial,
        "no tick before the interval"
    );

    state.animation.last_tick = std::time::Instant::now() - std::time::Duration::from_millis(150);
    state.tick_animation();
    assert_eq!(state.animation.frame, initial.wrapping_add(1));
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
    let dialog = state.dialogs.approval.as_ref().unwrap();
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
    assert_eq!(state.dialogs.approval.as_ref().unwrap().selected_index, 1);

    state.select_next_approval_option();
    assert_eq!(state.dialogs.approval.as_ref().unwrap().selected_index, 0);
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
    state.add_active_tool_call(
        "call1".to_string(),
        "bash".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );

    assert_eq!(state.tools.active.len(), 1);
    let tool = &state.tools.active[0];
    assert_eq!(tool.tool_call_id, "call1");
    assert_eq!(tool.display_name, "bash");
    assert_eq!(tool.status, ToolCallStatus::Starting);
}

#[test]
fn app_state_update_tool_call_status() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "call1".to_string(),
        "bash".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );

    state.update_tool_call_status("call1", ToolCallStatus::Executing);
    assert_eq!(state.tools.active[0].status, ToolCallStatus::Executing);
}

#[test]
fn app_state_set_tool_call_result() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "call1".to_string(),
        "bash".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );

    state.set_tool_call_result("call1", "success".to_string());
    assert_eq!(
        state.tools.active[0].result_summary,
        Some("success".to_string())
    );
}

#[test]
fn app_state_get_active_tool_call_mut() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "call1".to_string(),
        "bash".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );

    let tool = state.get_active_tool_call_mut("call1");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().display_name, "bash");
}

#[test]
fn app_state_complete_single_tool_call() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "call1".to_string(),
        "bash".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );
    state.set_tool_call_result("call1", "result".to_string());

    state.complete_single_tool_call("call1");
    assert!(state.tools.active.is_empty());
    assert!(state.has_pending_messages());
}

#[test]
fn toggle_display_compact_flips_and_returns_new_state() {
    let mut state = AppState::new();
    assert!(!state.display_compact);
    assert!(state.toggle_display_compact());
    assert!(state.display_compact);
    assert!(!state.toggle_display_compact());
    assert!(!state.display_compact);
}

fn rendered_text(state: &mut AppState) -> String {
    state
        .drain_pending_messages()
        .iter()
        .map(|line| match line {
            MessageLine::Plain(s) | MessageLine::Markdown(s) | MessageLine::Thinking(s) => {
                s.clone()
            }
            MessageLine::Styled(l) => l
                .spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<Vec<_>>()
                .join(""),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn add_thinking_suppressed_in_compact_mode() {
    let mut state = AppState::new();
    state.display_compact = true;
    state.add_thinking("a long internal monologue marker-xyz");
    assert!(!rendered_text(&mut state).contains("marker-xyz"));
}

#[test]
fn add_thinking_rendered_in_full_mode() {
    let mut state = AppState::new();
    state.add_thinking("a long internal monologue marker-xyz");
    assert!(rendered_text(&mut state).contains("marker-xyz"));
}

#[test]
fn complete_single_tool_call_skips_continuation_in_compact_mode() {
    let mut state = AppState::new();
    state.display_compact = true;
    state.add_active_tool_call(
        "call1".to_string(),
        "bash".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );
    state.set_tool_call_result("call1", "unique-result-marker".to_string());
    state.complete_single_tool_call("call1");
    assert!(!rendered_text(&mut state).contains("unique-result-marker"));
}

#[test]
fn complete_single_tool_call_includes_continuation_in_full_mode() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "call1".to_string(),
        "bash".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );
    state.set_tool_call_result("call1", "unique-result-marker".to_string());
    state.complete_single_tool_call("call1");
    assert!(rendered_text(&mut state).contains("unique-result-marker"));
}

#[test]
fn batch_completion_collapses_to_single_summary_in_scrollback() {
    let mut state = AppState::new();
    for (id, name) in [
        ("c1", "Read(a.txt)"),
        ("c2", "Read(b.txt)"),
        ("c3", "Read(c.txt)"),
    ] {
        state.add_active_tool_call(
            id.to_string(),
            name.to_string(),
            ToolRender::Standard,
            phrasing::READ,
        );
        state.update_tool_call_status(id, ToolCallStatus::Completed);
    }
    state.complete_active_tool_calls();
    state.handle_agent_event(AgentEvent::FinalResponse("done".into()));

    let rendered = rendered_text(&mut state);
    assert!(rendered.contains("Read 3 files"), "got: {rendered}");
    assert!(rendered.contains("a.txt, b.txt, c.txt"), "got: {rendered}");
    assert!(!rendered.contains("Read(a.txt)"), "got: {rendered}");
    assert!(state.tools.active.is_empty());
}

fn complete_batch(state: &mut AppState, calls: &[(&str, &str, CategoryPhrasing)]) {
    for (id, name, phrasing) in calls {
        state.add_active_tool_call(
            id.to_string(),
            name.to_string(),
            ToolRender::Standard,
            *phrasing,
        );
        state.update_tool_call_status(id, ToolCallStatus::Completed);
    }
    state.complete_active_tool_calls();
}

#[test]
fn consecutive_exploration_batches_coalesce_into_one_block() {
    let mut state = AppState::new();
    complete_batch(&mut state, &[("l1", "List(.)", phrasing::LIST)]);
    complete_batch(
        &mut state,
        &[
            ("r1", "Read(a.txt)", phrasing::READ),
            ("r2", "Read(b.txt)", phrasing::READ),
        ],
    );
    complete_batch(&mut state, &[("l2", "List(.hoosh)", phrasing::LIST)]);
    state.handle_agent_event(AgentEvent::FinalResponse("summary".into()));

    let rendered = rendered_text(&mut state);
    assert!(
        rendered.contains("Listed 2 directories, read 2 files"),
        "got: {rendered}"
    );
    assert_eq!(rendered.matches('⎿').count(), 1, "got: {rendered}");
    assert!(state.pending_exploration.is_empty());
}

#[test]
fn stream_start_without_text_does_not_seal_run() {
    let mut state = AppState::new();
    complete_batch(&mut state, &[("l1", "List(.)", phrasing::LIST)]);
    state.handle_agent_event(AgentEvent::StreamStarted);
    complete_batch(
        &mut state,
        &[
            ("r1", "Read(a.txt)", phrasing::READ),
            ("r2", "Read(b.txt)", phrasing::READ),
        ],
    );
    state.handle_agent_event(AgentEvent::FinalResponse("done".into()));

    let rendered = rendered_text(&mut state);
    assert!(
        rendered.contains("Listed 1 directory, read 2 files"),
        "got: {rendered}"
    );
    assert_eq!(rendered.matches('⎿').count(), 1, "got: {rendered}");
}

#[test]
fn text_delta_seals_pending_run() {
    let mut state = AppState::new();
    complete_batch(&mut state, &[("r1", "Read(a.txt)", phrasing::READ)]);
    state.handle_agent_event(AgentEvent::StreamStarted);
    state.handle_agent_event(AgentEvent::TextDelta("answer".into()));

    assert!(state.pending_exploration.is_empty());
    assert!(rendered_text(&mut state).contains("Read 1 file"));
}

#[test]
fn exploration_run_is_deferred_until_sealed() {
    let mut state = AppState::new();
    complete_batch(&mut state, &[("r1", "Read(a.txt)", phrasing::READ)]);

    assert!(rendered_text(&mut state).is_empty());
    assert_eq!(state.pending_exploration.len(), 1);
}

#[test]
fn non_exploration_batch_seals_prior_run_first() {
    let mut state = AppState::new();
    complete_batch(&mut state, &[("r1", "Read(a.txt)", phrasing::READ)]);
    complete_batch(&mut state, &[("e1", "Edit(a.txt)", phrasing::EDIT)]);

    let rendered = rendered_text(&mut state);
    let read_at = rendered
        .find("Read 1 file")
        .expect("read summary committed");
    let edit_at = rendered.find("Edit(a.txt)").expect("edit rendered");
    assert!(
        read_at < edit_at,
        "exploration must seal before edit: {rendered}"
    );
}

#[test]
fn expanded_view_opts_out_of_coalescing() {
    let mut state = AppState::new();
    state.tools.expanded = true;
    complete_batch(&mut state, &[("r1", "Read(a.txt)", phrasing::READ)]);

    assert!(rendered_text(&mut state).contains("Read(a.txt)"));
    assert!(state.pending_exploration.is_empty());
}

#[test]
fn expanded_batch_completion_keeps_per_call_lines() {
    let mut state = AppState::new();
    for (id, name) in [("c1", "Read(a.txt)"), ("c2", "Read(b.txt)")] {
        state.add_active_tool_call(
            id.to_string(),
            name.to_string(),
            ToolRender::Standard,
            phrasing::GENERIC,
        );
        state.update_tool_call_status(id, ToolCallStatus::Completed);
    }
    state.tools.expanded = true;
    state.complete_active_tool_calls();

    let rendered = rendered_text(&mut state);
    assert!(rendered.contains("Read(a.txt)"), "got: {rendered}");
    assert!(rendered.contains("Read(b.txt)"), "got: {rendered}");
}

#[test]
fn errored_batch_completion_stays_per_call() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "c1".into(),
        "Read(a.txt)".into(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );
    state.update_tool_call_status("c1", ToolCallStatus::Completed);
    state.add_active_tool_call(
        "c2".into(),
        "Read(b.txt)".into(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );
    state.update_tool_call_status("c2", ToolCallStatus::Error("boom".into()));
    state.complete_active_tool_calls();

    let rendered = rendered_text(&mut state);
    assert!(rendered.contains("Read(a.txt)"), "got: {rendered}");
    assert!(rendered.contains("boom"), "got: {rendered}");
}

#[test]
fn save_memory_renders_as_single_collapsed_line_in_full_mode() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "call1".to_string(),
        "SaveMemory(feedback: user-prefers-rust)".to_string(),
        ToolRender::Inline {
            prefix: "Saved memory: ",
        },
        phrasing::GENERIC,
    );
    state.set_tool_call_result("call1", "user_prefers_rust".to_string());
    state.update_tool_call_status("call1", ToolCallStatus::Completed);
    state.complete_single_tool_call("call1");
    let rendered = rendered_text(&mut state);
    assert!(rendered.contains("Saved memory: user_prefers_rust"));
    assert!(!rendered.contains("⎿"));
    assert!(!rendered.contains("SaveMemory("));
}

#[test]
fn save_memory_renders_as_single_collapsed_line_in_compact_mode() {
    let mut state = AppState::new();
    state.display_compact = true;
    state.add_active_tool_call(
        "call1".to_string(),
        "SaveMemory(feedback: user-prefers-rust)".to_string(),
        ToolRender::Inline {
            prefix: "Saved memory: ",
        },
        phrasing::GENERIC,
    );
    state.set_tool_call_result("call1", "user_prefers_rust".to_string());
    state.update_tool_call_status("call1", ToolCallStatus::Completed);
    state.complete_single_tool_call("call1");
    let rendered = rendered_text(&mut state);
    assert!(rendered.contains("Saved memory: user_prefers_rust"));
    assert!(!rendered.contains("⎿"));
}

#[test]
fn save_memory_error_falls_through_to_standard_render() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "call1".to_string(),
        "SaveMemory(feedback: bad)".to_string(),
        ToolRender::Inline {
            prefix: "Saved memory: ",
        },
        phrasing::GENERIC,
    );
    state.update_tool_call_status("call1", ToolCallStatus::Error("disk full".to_string()));
    state.complete_single_tool_call("call1");
    let rendered = rendered_text(&mut state);
    assert!(!rendered.contains("Saved memory:"));
}

#[test]
fn app_state_clear_active_tool_calls() {
    let mut state = AppState::new();
    state.add_active_tool_call(
        "call1".to_string(),
        "bash".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );
    state.add_active_tool_call(
        "call2".to_string(),
        "python".to_string(),
        ToolRender::Standard,
        phrasing::GENERIC,
    );

    state.clear_active_tool_calls();
    assert!(state.tools.active.is_empty());
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

fn scroll(content: usize, viewport: usize) -> ScrollState {
    ScrollState {
        content_length: content,
        viewport_length: viewport,
        ..Default::default()
    }
}

#[test]
fn scroll_down_clamps_to_max_offset() {
    let mut s = scroll(100, 10);
    assert_eq!(s.max_offset(), 90);
    s.down(50);
    assert_eq!(s.offset, 50);
    s.down(50);
    assert_eq!(s.offset, 90);
    s.down(50);
    assert_eq!(s.offset, 90);
}

#[test]
fn scroll_up_saturates_at_zero() {
    let mut s = scroll(100, 10);
    s.down(30);
    s.up(50);
    assert_eq!(s.offset, 0);
}

#[test]
fn scroll_page_sizes_derive_from_viewport() {
    let s = scroll(100, 10);
    assert_eq!(s.page(), 9);
    assert_eq!(s.half_page(), 5);
}

#[test]
fn scroll_at_bottom_reflects_max_offset() {
    let mut s = scroll(100, 10);
    assert!(!s.at_bottom());
    s.scroll_to_bottom();
    assert_eq!(s.offset, 90);
    assert!(s.at_bottom());
}

#[test]
fn scroll_clamp_pulls_offset_in_when_content_shrinks() {
    let mut s = scroll(100, 10);
    s.scroll_to_bottom();
    assert_eq!(s.offset, 90);
    s.content_length = 20;
    s.clamp();
    assert_eq!(s.offset, 10);
}

fn executing_call(id: &str, render: ToolRender) -> ActiveToolCall {
    ActiveToolCall {
        tool_call_id: id.to_string(),
        display_name: format!("call {id}"),
        render,
        phrasing: phrasing::GENERIC,
        status: ToolCallStatus::Executing,
        preview: None,
        budget_pct: None,
        result_summary: None,
        subagent_steps: Vec::new(),
        is_subagent_task: false,
        bash_output_lines: Vec::new(),
        is_bash_streaming: false,
        start_time: Instant::now(),
        total_tool_uses: None,
        total_tokens: None,
    }
}

#[test]
fn standard_batch_collapses() {
    let mut state = AppState::new();
    state.tools.active = vec![
        executing_call("a", ToolRender::Standard),
        executing_call("b", ToolRender::Standard),
    ];
    assert!(state.tool_calls_collapsed());
}

#[test]
fn subagent_batch_does_not_collapse() {
    let mut state = AppState::new();
    state.tools.active = vec![
        executing_call("a", ToolRender::Subagent),
        executing_call("b", ToolRender::Subagent),
    ];
    assert!(
        !state.tool_calls_collapsed(),
        "concurrent subagents must keep their individual rows and completion stats"
    );
}

#[test]
fn mixed_standard_and_subagent_does_not_collapse() {
    let mut state = AppState::new();
    state.tools.active = vec![
        executing_call("a", ToolRender::Standard),
        executing_call("b", ToolRender::Subagent),
    ];
    assert!(!state.tool_calls_collapsed());
}

#[test]
fn subagent_completion_preserves_individual_stats() {
    let mut state = AppState::new();
    let mut a = executing_call("a", ToolRender::Subagent);
    a.is_subagent_task = true;
    a.status = ToolCallStatus::Completed;
    a.total_tool_uses = Some(3);
    a.total_tokens = Some(1500);
    let mut b = executing_call("b", ToolRender::Subagent);
    b.is_subagent_task = true;
    b.status = ToolCallStatus::Completed;
    b.total_tool_uses = Some(2);
    b.total_tokens = Some(900);
    state.tools.active = vec![a, b];

    state.complete_active_tool_calls();

    let rendered = state
        .messages
        .iter()
        .map(|m| match m {
            MessageLine::Plain(t) => t.clone(),
            MessageLine::Styled(line) => line.spans.iter().map(|s| s.content.as_ref()).collect(),
            MessageLine::Markdown(t) => t.clone(),
            MessageLine::Thinking(t) => t.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("3 tool uses") && rendered.contains("2 tool uses"),
        "each subagent should report its own stats, got:\n{rendered}"
    );
}
