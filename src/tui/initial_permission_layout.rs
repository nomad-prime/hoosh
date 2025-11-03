use crate::tui::app::AppState;
use crate::tui::app_layout_builder::AppLayoutBuilder;
use crate::tui::layout::Layout;
use crate::tui::layout_builder::LayoutBuilder;

pub trait InitialPermissionLayout {
    fn create(app: &AppState) -> Self;
}

impl InitialPermissionLayout for Layout<AppState> {
    fn create(app: &AppState) -> Self {
        let mut builder = LayoutBuilder::new();

        if app.is_showing_initial_permission_dialog() {
            builder = builder.initial_permission_dialog(true);
        }

        builder.build()
    }
}
