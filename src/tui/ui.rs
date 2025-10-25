use super::app::AppState;
use super::components::{
    approval_dialog::ApprovalDialogWidget, completion_popup::CompletionPopupWidget,
    input::InputWidget, mode_indicator::ModeIndicatorWidget,
    permission_dialog::PermissionDialogWidget, status::StatusWidget,
};
use crate::tui::minimal_terminal::Frame;
use ratatui::layout::{Constraint, Layout};

/// Calculates the total vertical height needed for the entire UI.
/// This is called by `draw_dynamic_ui` *before* the draw call.
pub fn calculate_desired_height(app: &AppState) -> u16 {
    // --- Define your component heights ---
    // These are the fixed heights of your UI components
    let status_height = 1;
    let input_height = 3; // From your original file (Borders::BOTTOM | Borders::TOP + 1 line text)
    let mode_height = 1;

    // --- Calculate dynamic dialog height ---
    // This logic determines how much *extra* space is needed for popups.
    // You should make these values more precise.
    let dialog_height = if app.is_showing_permission_dialog() {
        15 // Or calculate based on app_state.permission_dialog_state.options.len()
    } else if app.is_showing_approval_dialog() {
        10 // Or calculate based on app_state.approval_dialog_state.options.len()
    } else if app.is_completing() {
        12 // Or calculate based on app_state.completion_state.candidates.len()
    } else {
        0
    };

    // --- Calculate total height ---
    let base_ui_height = if dialog_height > 0 {
        // If a dialog is showing, we DON'T show the mode indicator
        status_height + input_height
    } else {
        // If no dialog, we show the mode indicator
        status_height + input_height + mode_height
    };

    // Total height is the base UI + the dynamic dialog height
    base_ui_height + dialog_height
}

/// Renders the entire UI into a frame.
/// This is called *inside* `draw_dynamic_ui`.
/// It assumes the frame it receives is *already* the correct size.
pub fn render_ui(frame: &mut Frame, app: &mut AppState) {
    let viewport_area = frame.area();

    // Re-calculate the dialog height (this is fast)
    let dialog_height = if app.is_showing_permission_dialog() {
        15
    } else if app.is_showing_approval_dialog() {
        10
    } else if app.is_completing() {
        12
    } else {
        0
    };

    // --- Create Layout ---
    // We create a layout that *perfectly* fills the frame,
    // since the frame is already the correct height.
    let (status_area, input_area, bottom_area) = if dialog_height > 0 {
        // Layout with a dialog
        let vertical = Layout::vertical([
            Constraint::Length(1),             // status_area
            Constraint::Length(3),             // input_area
            Constraint::Length(dialog_height), // dialog_area
        ]);
        let [status, input, dialog] = vertical.areas(viewport_area);
        (status, input, dialog)
    } else {
        // Layout without a dialog (shows mode indicator)
        let vertical = Layout::vertical([
            Constraint::Length(1), // status_area
            Constraint::Length(3), // input_area
            Constraint::Length(1), // mode_area
        ]);
        let [status, input, mode] = vertical.areas(viewport_area);
        (status, input, mode)
    };

    // --- Render Widgets ---

    frame.render_widget(StatusWidget::new(app), status_area);
    frame.render_widget(InputWidget::new(&app.input), input_area);

    // Render the correct widget in the bottom area
    if app.is_completing() {
        frame.render_widget(CompletionPopupWidget::new(app, input_area), bottom_area);
    } else if app.is_showing_permission_dialog() {
        frame.render_widget(PermissionDialogWidget::new(app, input_area), bottom_area);
    } else if app.is_showing_approval_dialog() {
        frame.render_widget(ApprovalDialogWidget::new(app, input_area), bottom_area);
    } else {
        // No dialogs, just show the mode indicator
        frame.render_widget(ModeIndicatorWidget::new(app), bottom_area);
    }
}
