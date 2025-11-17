use crate::tui::app_layout_builder::AppLayoutBuilder;
use crate::tui::app_state::AppState;
use crate::tui::layout::Layout;
use crate::tui::layout_builder::LayoutBuilder;

pub trait AppLayout {
    fn create(app: &AppState) -> Self;
}

impl AppLayout for Layout<AppState> {
    fn create(app: &AppState) -> Self {
        let has_overlay = app.is_showing_tool_permission_dialog()
            || app.is_showing_approval_dialog()
            || app.is_completing();

        let active_tool_calls_visible = !app.active_tool_calls.is_empty();
        let active_tool_calls_height = app.active_tool_calls.iter().fold(0u16, |acc, tc| {
            let mut height = 1u16;
            if tc.result_summary.is_some() {
                height += 1;
            }
            acc + height
        });

        // Calculate subagent results visibility and height
        let has_subagent_tasks = app.active_tool_calls.iter().any(|tc| tc.is_subagent_task);
        let subagent_results_visible = has_subagent_tasks;
        let subagent_results_height = app.active_tool_calls.iter().fold(0u16, |acc, tc| {
            if !tc.is_subagent_task || tc.subagent_steps.is_empty() {
                return acc;
            }
            const MAX_STEPS: usize = 5;
            let total_steps = tc.subagent_steps.len();
            let steps_to_show = total_steps.min(MAX_STEPS);
            let mut height = steps_to_show as u16;

            // Add 1 for ellipsis if there are more steps
            if total_steps > MAX_STEPS {
                height += 1;
            }
            acc + height
        });

        let mut builder = LayoutBuilder::new()
            .spacer(1)
            .active_tool_calls(active_tool_calls_height, active_tool_calls_visible)
            .subagent_results(subagent_results_height, subagent_results_visible)
            .spacer_if(1, active_tool_calls_visible || subagent_results_visible)
            .status_bar()
            .input_field()
            .mode_indicator(!has_overlay);

        if app.is_showing_tool_permission_dialog() {
            let lines = app
                .tool_permission_dialog_state
                .as_ref()
                .map(|state| {
                    let mut base = 4 + state.options.len() as u16;

                    // Add height for command preview if present
                    if let Some(preview) = state.descriptor.command_preview() {
                        // +3 for top border, bottom border, and spacing
                        // + number of lines in the preview
                        let preview_lines = preview.lines().count() as u16;
                        base += 3 + preview_lines;
                    }

                    if state.descriptor.is_destructive() {
                        base + 1
                    } else {
                        base
                    }
                })
                .unwrap_or(10);
            builder = builder.permission_dialog(lines.min(30), true);
        } else if app.is_showing_approval_dialog() {
            builder = builder.approval_dialog(true);
        } else if app.is_completing() {
            let lines = app
                .completion_state
                .as_ref()
                .map(|state| state.candidates.len().min(10) as u16)
                .unwrap_or(5);
            builder = builder.completion_popup(lines, true);
        }

        builder.build()
    }
}
