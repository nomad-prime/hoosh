use super::*;
use crate::tools::{CategoryPhrasing, ToolRender, phrasing};
use crate::tui::app_state::{ActiveToolCall, ToolCallStatus};
use std::time::Instant;

fn call(display_name: &str, phrasing: CategoryPhrasing) -> ActiveToolCall {
    ActiveToolCall {
        tool_call_id: display_name.to_string(),
        display_name: display_name.to_string(),
        render: ToolRender::Standard,
        phrasing,
        status: ToolCallStatus::Executing,
        preview: None,
        result_summary: None,
        subagent_steps: Vec::new(),
        is_subagent_task: false,
        bash_output_lines: Vec::new(),
        is_bash_streaming: false,
        start_time: Instant::now(),
        budget_pct: None,
        total_tool_uses: None,
        total_tokens: None,
    }
}

#[test]
fn single_read_uses_singular_noun() {
    let calls = vec![call("Read(src/main.rs)", phrasing::READ)];
    assert_eq!(aggregate_phrase(&calls), "Reading 1 file");
}

#[test]
fn multiple_reads_pluralize() {
    let calls = vec![
        call("Read(a.rs)", phrasing::READ),
        call("Read(b.rs)", phrasing::READ),
        call("Read(c.rs)", phrasing::READ),
    ];
    assert_eq!(aggregate_phrase(&calls), "Reading 3 files");
}

#[test]
fn mixed_batch_preserves_first_seen_order() {
    let calls = vec![
        call("Grep(needle)", phrasing::SEARCH),
        call("Read(a.rs)", phrasing::READ),
        call("Read(b.rs)", phrasing::READ),
    ];
    assert_eq!(
        aggregate_phrase(&calls),
        "Searching for 1 pattern, reading 2 files"
    );
}

#[test]
fn list_directory_pluralizes_irregularly() {
    let calls = vec![
        call("List(src)", phrasing::LIST),
        call("List(tests)", phrasing::LIST),
    ];
    assert_eq!(aggregate_phrase(&calls), "Listing 2 directories");
}

#[test]
fn subagents_aggregate_as_agents() {
    let calls = vec![
        call("Explore (find X)", phrasing::SUBAGENT),
        call("Explore (find Y)", phrasing::SUBAGENT),
    ];
    assert_eq!(aggregate_phrase(&calls), "Running 2 agents");
}

#[test]
fn unknown_tool_falls_back_to_generic_phrase() {
    let calls = vec![call("Frobnicate(x)", phrasing::GENERIC)];
    assert_eq!(aggregate_phrase(&calls), "Running 1 tool");
}

#[test]
fn completed_batch_uses_past_tense() {
    let calls = vec![
        call("Grep(a)", phrasing::SEARCH),
        call("Grep(b)", phrasing::SEARCH),
        call("Read(x.rs)", phrasing::READ),
        call("Read(y.rs)", phrasing::READ),
        call("Read(z.rs)", phrasing::READ),
        call("Read(w.rs)", phrasing::READ),
        call("List(src)", phrasing::LIST),
    ];
    assert_eq!(
        completed_phrase(&calls),
        "Searched for 2 patterns, read 4 files, listed 1 directory"
    );
}

#[test]
fn completed_single_read_uses_past_tense() {
    let calls = vec![call("Read(src/main.rs)", phrasing::READ)];
    assert_eq!(completed_phrase(&calls), "Read 1 file");
}

#[test]
fn basenames_strip_directories() {
    let calls = vec![
        call("Read(/Users/dev/proj/MEMORY.md)", phrasing::READ),
        call("Read(src/tools/read_file.rs)", phrasing::READ),
    ];
    assert_eq!(
        target_basenames(&calls),
        vec!["MEMORY.md".to_string(), "read_file.rs".to_string()]
    );
}

#[test]
fn basename_without_parens_returns_display_name() {
    let calls = vec![call("Bash", phrasing::RUN)];
    assert_eq!(target_basenames(&calls), vec!["Bash".to_string()]);
}
