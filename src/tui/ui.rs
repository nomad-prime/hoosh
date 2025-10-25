use super::app::AppState;
use super::components::{
    approval_dialog::ApprovalDialogWidget, completion_popup::CompletionPopupWidget,
    input::InputWidget, mode_indicator::ModeIndicatorWidget,
    permission_dialog::PermissionDialogWidget, status::StatusWidget,
};
use crate::tui::layout_builder::Layout;
use crate::tui::terminal::Frame;
use ratatui::layout::{Constraint, Layout as RatatuiLayout};

/// Renders the entire UI into a frame.
/// This is called *inside* `draw_dynamic_ui`.
/// It assumes the frame it receives is *already* the correct size.
pub fn render_ui(frame: &mut Frame, app: &mut AppState, layout: &Layout) {
    let viewport_area = frame.area();

    // Build constraints from the layout
    let constraints: Vec<Constraint> = layout
        .visible_components()
        .map(|comp| Constraint::Length(comp.height()))
        .collect();

    // Create the vertical layout
    let areas = RatatuiLayout::vertical(constraints).split(viewport_area);

    // Render each visible component
    let mut area_idx = 0;
    let mut input_area = None;

    for component in layout.visible_components() {
        let area = areas[area_idx];

        match component.name {
            "status" => {
                frame.render_widget(StatusWidget::new(app), area);
            }
            "input" => {
                frame.render_widget(InputWidget::new(&app.input), area);
                input_area = Some(area); // Save for dialog anchoring
            }
            "mode" => {
                frame.render_widget(ModeIndicatorWidget::new(app), area);
            }
            "permission" => {
                if let Some(anchor) = input_area {
                    frame.render_widget(PermissionDialogWidget::new(app, anchor), area);
                }
            }
            "approval" => {
                if let Some(anchor) = input_area {
                    frame.render_widget(ApprovalDialogWidget::new(app, anchor), area);
                }
            }
            "completion" => {
                if let Some(anchor) = input_area {
                    frame.render_widget(CompletionPopupWidget::new(app, anchor), area);
                }
            }
            _ => {}
        }

        area_idx += 1;
    }
}
