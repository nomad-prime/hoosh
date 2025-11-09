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
            || app.is_showing_initial_permission_dialog()
            || app.is_completing();

        let mut builder = LayoutBuilder::new()
            .spacer(1)
            .status_bar()
            .input_field()
            .mode_indicator(!has_overlay);

        if app.is_showing_initial_permission_dialog() {
            builder = builder.initial_permission_dialog(true);
        } else if app.is_showing_tool_permission_dialog() {
            let lines = app
                .tool_permission_dialog_state
                .as_ref()
                .map(|state| {
                    let base = 4 + state.options.len() as u16;
                    if state.descriptor.is_destructive() {
                        base + 1
                    } else {
                        base
                    }
                })
                .unwrap_or(10);
            builder = builder.permission_dialog(lines.min(15), true);
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
