use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

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

    #[allow(dead_code)]
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

pub trait WidgetRenderer: Send + Sync {
    type State;
    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer);
}

pub struct ComponentDescriptor<S> {
    pub box_model: BoxModel,
    pub visible: bool,
    pub renderer: Option<Box<dyn WidgetRenderer<State = S>>>,
}

impl<S> ComponentDescriptor<S> {
    pub fn new(height: u16, renderer: Option<Box<dyn WidgetRenderer<State = S>>>) -> Self {
        Self {
            box_model: BoxModel::new(height),
            visible: true,
            renderer,
        }
    }

    pub fn with_border(mut self) -> Self {
        self.box_model = self.box_model.with_border(1, 1);
        self
    }

    #[allow(dead_code)]
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

pub struct Layout<S> {
    components: Vec<ComponentDescriptor<S>>,
}

impl<S> Layout<S> {
    pub fn total_height(&self) -> u16 {
        self.components.iter().map(|c| c.height()).sum()
    }

    pub fn visible_components(&self) -> impl Iterator<Item = &ComponentDescriptor<S>> {
        self.components.iter().filter(|c| c.visible)
    }

    pub fn render(&self, state: &S, area: Rect, buf: &mut Buffer) {
        use ratatui::layout::{Constraint, Layout as RatatuiLayout};

        let constraints: Vec<Constraint> = self
            .visible_components()
            .map(|comp| Constraint::Length(comp.height()))
            .collect();

        let areas = RatatuiLayout::vertical(constraints).split(area);

        for (area_idx, component) in self.visible_components().enumerate() {
            let component_area = areas[area_idx];

            if let Some(renderer) = &component.renderer {
                renderer.render(state, component_area, buf);
            }
        }
    }
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
