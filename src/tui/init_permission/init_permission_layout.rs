use crate::tui::layout::{ComponentDescriptor, Layout};
use crate::tui::layout_builder::LayoutBuilder;
use super::init_permission_dialog::InitialPermissionDialog;
use super::init_permission_state::InitialPermissionState;

pub trait InitialPermissionLayout {
    fn create(app: &InitialPermissionState) -> Self;
}

impl InitialPermissionLayout for Layout<InitialPermissionState> {
    fn create(_app: &InitialPermissionState) -> Self {
        let mut builder = LayoutBuilder::new();

        builder = builder.component(
            ComponentDescriptor::new(25, Some(Box::new(InitialPermissionDialog))).with_border(),
        );

        builder.build()
    }
}
