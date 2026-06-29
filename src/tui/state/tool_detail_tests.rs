use super::*;

fn render(detail: &dyn ToolDetail, expanded: bool) -> String {
    detail
        .detail_lines(expanded)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn bash(num_lines: usize) -> BashDetail {
    BashDetail {
        lines: (0..num_lines)
            .map(|i| BashOutputLine {
                line_number: i + 1,
                content: format!("line {i}"),
                stream_type: "stdout".into(),
            })
            .collect(),
    }
}

fn subagent(num_steps: usize) -> SubagentDetail {
    SubagentDetail {
        steps: (0..num_steps)
            .map(|i| SubagentStepSummary {
                step_number: i + 1,
                action_type: "tool_starting".into(),
                description: format!("step {i}"),
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn bash_collapsed_shows_five_lines_with_expand_hint() {
    let out = render(&bash(8), false);
    assert!(
        out.contains("ctrl+o to expand"),
        "missing expand hint:\n{out}"
    );
    assert!(out.contains("(+3 lines"), "wrong hidden count:\n{out}");
    // Newest 5 of 8 are lines 3..=7; lines 1 and 2 must be hidden.
    assert!(out.contains("line 7"));
    assert!(!out.contains("line 1\n") && !out.contains("line 2\n"));
}

#[test]
fn bash_expanded_shows_more_lines_with_collapse_hint() {
    let out = render(&bash(35), true);
    assert!(
        out.contains("ctrl+o to collapse"),
        "missing collapse hint:\n{out}"
    );
    assert!(out.contains("(+5 lines"), "wrong hidden count:\n{out}");
}

#[test]
fn bash_no_hint_when_output_fits() {
    let out = render(&bash(3), false);
    assert!(!out.contains("ctrl+o"), "unexpected hint:\n{out}");
}

#[test]
fn bash_boundary_marks_first_visible_row_when_no_overflow() {
    let out = render(&bash(2), false);
    assert!(
        out.starts_with("  ⎿ "),
        "first row should carry boundary:\n{out}"
    );
}

#[test]
fn bash_boundary_marks_overflow_hint_when_present() {
    let out = render(&bash(8), false);
    assert!(
        out.starts_with("  ⎿ ... (+3 lines"),
        "overflow hint should carry the boundary:\n{out}"
    );
}

#[test]
fn subagent_shows_last_five_steps_with_bottom_ellipsis() {
    let out = render(&subagent(8), false);
    assert!(
        out.starts_with("  ⎿ "),
        "first step should carry boundary:\n{out}"
    );
    assert!(out.contains("step 7"));
    assert!(!out.contains("step 0") && !out.contains("step 2"));
    assert!(
        out.trim_end().ends_with("..."),
        "ellipsis goes last:\n{out}"
    );
}

#[test]
fn subagent_no_ellipsis_when_within_window() {
    let out = render(&subagent(3), false);
    assert!(!out.contains("..."), "unexpected ellipsis:\n{out}");
    assert_eq!(out.lines().count(), 3);
}

#[test]
fn subagent_ignores_expansion() {
    assert_eq!(render(&subagent(8), false), render(&subagent(8), true));
}
