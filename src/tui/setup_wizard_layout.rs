use crate::tui::components::SetupWizardDialog;
use crate::tui::layout::{ComponentDescriptor, Layout};
use crate::tui::layout_builder::LayoutBuilder;
use crate::tui::setup_wizard_app::SetupWizardApp;

pub trait SetupWizardLayout {
    fn create(app: &SetupWizardApp) -> Self;
}

impl SetupWizardLayout for Layout<SetupWizardApp> {
    fn create(_app: &SetupWizardApp) -> Self {
        let mut builder = LayoutBuilder::new();

        builder = builder.component(
            ComponentDescriptor::new(25, Some(Box::new(SetupWizardDialog))).with_border(),
        );

        builder.build()
    }
}
