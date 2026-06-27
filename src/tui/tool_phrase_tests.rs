use super::*;
use crate::tools::ToolRender;
use crate::tui::app_state::{ActiveToolCall, ToolCallStatus};
use std::time::Instant;

fn call(display_name: &str) -> ActiveToolCall {
    ActiveToolCall {
        tool_call_id: display_name.to_string(),
        display_name: display_name.to_string(),
        render: ToolRender::Standard,
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
    let calls = vec![call("Read(src/main.rs)")];
    assert_eq!(aggregate_phrase(&calls), "Reading 1 file");
}

#[test]
fn multiple_reads_pluralize() {
    let calls = vec![call("Read(a.rs)"), call("Read(b.rs)"), call("Read(c.rs)")];
    assert_eq!(aggregate_phrase(&calls), "Reading 3 files");
}

#[test]
fn mixed_batch_preserves_first_seen_order() {
    let calls = vec![call("Grep(needle)"), call("Read(a.rs)"), call("Read(b.rs)")];
    assert_eq!(
        aggregate_phrase(&calls),
        "Searching for 1 pattern, reading 2 files"
    );
}

#[test]
fn list_directory_pluralizes_irregularly() {
    let calls = vec![call("List(src)"), call("List(tests)")];
    assert_eq!(aggregate_phrase(&calls), "Listing 2 directories");
}

#[test]
fn unknown_tool_falls_back_to_generic_phrase() {
    let calls = vec![call("Frobnicate(x)")];
    assert_eq!(aggregate_phrase(&calls), "Running 1 tool");
}

#[test]
fn basenames_strip_directories() {
    let calls = vec![
        call("Read(/Users/dev/proj/MEMORY.md)"),
        call("Read(src/tools/read_file.rs)"),
    ];
    assert_eq!(
        target_basenames(&calls),
        vec!["MEMORY.md".to_string(), "read_file.rs".to_string()]
    );
}

#[test]
fn basename_without_parens_returns_display_name() {
    let calls = vec![call("Bash")];
    assert_eq!(target_basenames(&calls), vec!["Bash".to_string()]);
}
