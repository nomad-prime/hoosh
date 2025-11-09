use crate::tui::app_state::AppState;
use crate::tui::components::approval_dialog::ApprovalDialog;
use crate::tui::components::completion_popup::CompletionPopup;
use crate::tui::components::input::Input;
use crate::tui::components::mode_indicator::ModeIndicator;
use crate::tui::components::permission_dialog::PermissionDialog;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::layout::ComponentDescriptor;
use crate::tui::layout_builder::LayoutBuilder;

pub trait AppLayoutBuilder {
    fn status_bar(self) -> Self;
    fn input_field(self) -> Self;
    fn mode_indicator(self, visible: bool) -> Self;
    fn permission_dialog(self, content_lines: u16, visible: bool) -> Self;
    fn approval_dialog(self, visible: bool) -> Self;
    fn completion_popup(self, content_lines: u16, visible: bool) -> Self;
}

impl AppLayoutBuilder for LayoutBuilder<AppState> {
    fn status_bar(self) -> Self {
        self.component(ComponentDescriptor::new(1, Some(Box::new(StatusBar))))
    }

    fn input_field(self) -> Self {
        self.component(ComponentDescriptor::new(1, Some(Box::new(Input))).with_border())
    }

    fn mode_indicator(self, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new(1, Some(Box::new(ModeIndicator))).with_visibility(visible),
        )
    }

    fn permission_dialog(self, content_lines: u16, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new(content_lines, Some(Box::new(PermissionDialog)))
                .with_border()
                .with_visibility(visible),
        )
    }

    fn approval_dialog(self, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new(6, Some(Box::new(ApprovalDialog)))
                .with_border()
                .with_visibility(visible),
        )
    }

    fn completion_popup(self, content_lines: u16, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new(content_lines, Some(Box::new(CompletionPopup)))
                .with_border()
                .with_visibility(visible),
        )
    }
}
