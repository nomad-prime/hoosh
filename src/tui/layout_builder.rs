use crate::tui::app::AppState;

pub trait Measurable {
    fn measure_height(&self, app: &AppState) -> u16;

    fn is_visible(&self, app: &AppState) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxModel {
    pub content: u16,
    pub padding_top: u16,
    pub padding_bottom: u16,
    pub border_top: u16,
    pub border_bottom: u16,
}

impl BoxModel {
    pub const fn new(content: u16) -> Self {
        Self {
            content,
            padding_top: 0,
            padding_bottom: 0,
            border_top: 0,
            border_bottom: 0,
        }
    }

    pub const fn with_padding(mut self, top: u16, bottom: u16) -> Self {
        self.padding_top = top;
        self.padding_bottom = bottom;
        self
    }

    pub const fn with_border(mut self, top: u16, bottom: u16) -> Self {
        self.border_top = top;
        self.border_bottom = bottom;
        self
    }

    pub const fn total_height(&self) -> u16 {
        self.content + self.padding_top + self.padding_bottom + self.border_top + self.border_bottom
    }
}

#[derive(Clone)]
pub struct ComponentDescriptor {
    pub name: &'static str,
    pub box_model: BoxModel,
    pub visible: bool,
}

impl ComponentDescriptor {
    pub fn new(name: &'static str, height: u16) -> Self {
        Self {
            name,
            box_model: BoxModel::new(height),
            visible: true,
        }
    }

    pub fn with_border(mut self) -> Self {
        self.box_model = self.box_model.with_border(1, 1);
        self
    }

    pub fn with_padding(mut self, top: u16, bottom: u16) -> Self {
        self.box_model = self.box_model.with_padding(top, bottom);
        self
    }

    pub fn with_visibility(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn height(&self) -> u16 {
        if self.visible {
            self.box_model.total_height()
        } else {
            0
        }
    }
}

pub struct LayoutBuilder {
    components: Vec<ComponentDescriptor>,
}

impl LayoutBuilder {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    pub fn spacer(self, name: &'static str, height: u16) -> Self {
        self.component(ComponentDescriptor::new(name, height))
    }

    /// Generic method - add ANY component
    pub fn component(mut self, desc: ComponentDescriptor) -> Self {
        self.components.push(desc);
        self
    }

    /// Generic method - calculate height
    pub fn total_height(&self) -> u16 {
        self.components.iter().map(|c| c.height()).sum()
    }

    /// Generic method - build the layout
    pub fn build(self) -> Layout {
        Layout {
            components: self.components,
        }
    }
}

impl Default for LayoutBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Layout {
    components: Vec<ComponentDescriptor>,
}

impl Layout {
    pub fn total_height(&self) -> u16 {
        self.components.iter().map(|c| c.height()).sum()
    }

    pub fn get_component(&self, name: &str) -> Option<&ComponentDescriptor> {
        self.components.iter().find(|c| c.name == name)
    }

    pub fn visible_components(&self) -> impl Iterator<Item = &ComponentDescriptor> {
        self.components.iter().filter(|c| c.visible)
    }
}

pub fn layout() -> LayoutBuilder {
    LayoutBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_model_calculation() {
        let box_model = BoxModel::new(5).with_padding(1, 1).with_border(1, 1);

        assert_eq!(box_model.total_height(), 9); // 5 + 1 + 1 + 1 + 1
    }
}
