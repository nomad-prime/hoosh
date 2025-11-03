use crate::tui::layout::{ComponentDescriptor, Layout};

pub struct LayoutBuilder<S> {
    components: Vec<ComponentDescriptor<S>>,
}

impl<S> LayoutBuilder<S> {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    pub fn spacer(self, height: u16) -> Self {
        self.component(ComponentDescriptor::new(height, None))
    }

    pub fn component(mut self, desc: ComponentDescriptor<S>) -> Self {
        self.components.push(desc);
        self
    }

    /// Generic method - build the layout
    pub fn build(self) -> Layout<S> {
        Layout {
            components: self.components,
        }
    }
}

impl<S> Default for LayoutBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}
