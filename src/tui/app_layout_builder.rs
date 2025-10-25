use crate::tui::app::AppState;
use crate::tui::components::approval_dialog::ApprovalDialogRenderer;
use crate::tui::components::completion_popup::CompletionPopupRenderer;
use crate::tui::components::input::InputRenderer;
use crate::tui::components::mode_indicator::ModeIndicatorRenderer;
use crate::tui::components::permission_dialog::PermissionDialogRenderer;
use crate::tui::components::status::StatusRenderer;
use crate::tui::layout_builder::{ComponentDescriptor, LayoutBuilder};

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
        self.component(ComponentDescriptor::new(1, Some(Box::new(StatusRenderer))))
    }

    fn input_field(self) -> Self {
        self.component(ComponentDescriptor::new(1, Some(Box::new(InputRenderer))).with_border())
    }

    fn mode_indicator(self, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new(1, Some(Box::new(ModeIndicatorRenderer)))
                .with_visibility(visible),
        )
    }

    fn permission_dialog(self, content_lines: u16, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new(content_lines, Some(Box::new(PermissionDialogRenderer)))
                .with_border()
                .with_visibility(visible),
        )
    }

    fn approval_dialog(self, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new(6, Some(Box::new(ApprovalDialogRenderer)))
                .with_border()
                .with_visibility(visible),
        )
    }

    fn completion_popup(self, content_lines: u16, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new(content_lines, Some(Box::new(CompletionPopupRenderer)))
                .with_border()
                .with_visibility(visible),
        )
    }
}
