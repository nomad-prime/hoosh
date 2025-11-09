use crate::tui::layout::{ComponentDescriptor, Layout};
use crate::tui::layout_builder::LayoutBuilder;
use crate::tui::setup::setup_wizard_dialog::SetupWizardDialog;
use crate::tui::setup::setup_wizard_state::SetupWizardState;

pub trait SetupWizardLayout {
    fn create(app: &SetupWizardState) -> Self;
}

impl SetupWizardLayout for Layout<SetupWizardState> {
    fn create(_app: &SetupWizardState) -> Self {
        let mut builder = LayoutBuilder::new();

        builder = builder.component(
            ComponentDescriptor::new(25, Some(Box::new(SetupWizardDialog))).with_border(),
        );

        builder.build()
    }
}
