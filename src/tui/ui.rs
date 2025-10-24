use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};

use super::app::AppState;
use super::components::{
    approval_dialog::ApprovalDialogWidget, completion_popup::CompletionPopupWidget,
    input::InputWidget, mode_indicator::ModeIndicatorWidget,
    permission_dialog::PermissionDialogWidget, status::StatusWidget,
};

pub fn render(frame: &mut Frame, app: &mut AppState) {
    let viewport_area = frame.area();

    let dialog_height = if app.is_showing_permission_dialog() {
        15
    } else if app.is_showing_approval_dialog() {
        10
    } else if app.is_completing() {
        12
    } else {
        0
    };

    let vertical = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(dialog_height),
    ]);

    let [_spacer, status_area, input_area, mode_area, dialog_area] = vertical.areas(viewport_area);

    frame.render_widget(StatusWidget::new(app), status_area);
    frame.render_widget(InputWidget::new(&app.input), input_area);

    if !app.is_completing()
        && !app.is_showing_permission_dialog()
        && !app.is_showing_approval_dialog()
    {
        frame.render_widget(ModeIndicatorWidget::new(app), mode_area);
    }

    if app.is_completing() {
        frame.render_widget(CompletionPopupWidget::new(app, input_area), dialog_area);
    }

    if app.is_showing_permission_dialog() {
        frame.render_widget(PermissionDialogWidget::new(app, mode_area), dialog_area);
    }

    if app.is_showing_approval_dialog() {
        frame.render_widget(ApprovalDialogWidget::new(app, mode_area), dialog_area);
    }
}
