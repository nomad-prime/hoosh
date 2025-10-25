use crate::tui::layout_builder::{ComponentDescriptor, LayoutBuilder};

pub trait AppLayoutBuilder {
    fn status_bar(self) -> Self;
    fn input_field(self) -> Self;
    fn mode_indicator(self, visible: bool) -> Self;
    fn permission_dialog(self, content_lines: u16, visible: bool) -> Self;
    fn approval_dialog(self, visible: bool) -> Self;
    fn completion_popup(self, content_lines: u16, visible: bool) -> Self;
}

impl AppLayoutBuilder for LayoutBuilder {
    fn status_bar(self) -> Self {
        self.component(ComponentDescriptor::new("status", 1))
    }

    fn input_field(self) -> Self {
        self.component(ComponentDescriptor::new("input", 1).with_border())
    }

    fn mode_indicator(self, visible: bool) -> Self {
        self.component(ComponentDescriptor::new("mode", 1).with_visibility(visible))
    }

    fn permission_dialog(self, content_lines: u16, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new("permission", content_lines)
                .with_border()
                .with_visibility(visible),
        )
    }

    fn approval_dialog(self, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new("approval", 6)
                .with_border()
                .with_visibility(visible),
        )
    }

    fn completion_popup(self, content_lines: u16, visible: bool) -> Self {
        self.component(
            ComponentDescriptor::new("completion", content_lines)
                .with_border()
                .with_visibility(visible),
        )
    }
}
