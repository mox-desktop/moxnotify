use glyphon::FontSystem;
use taffy::TaffyTree;

use crate::rendering::text_renderer;

pub trait GlobalLayout {
    fn global_layout(&self, node: taffy::NodeId) -> taffy::TaffyResult<taffy::Layout>;
}

impl GlobalLayout for TaffyTree<()> {
    fn global_layout(&self, node: taffy::NodeId) -> taffy::TaffyResult<taffy::Layout> {
        let mut current_node = node;
        let mut global_layout = self.layout(node)?.clone();

        while let Some(parent) = self.parent(current_node) {
            let parent_layout = self.layout(parent)?;
            global_layout.location.x += parent_layout.location.x + parent_layout.margin.left;
            global_layout.location.y += parent_layout.location.y + parent_layout.margin.top;
            current_node = parent;
        }

        Ok(global_layout)
    }
}

pub enum NodeContext {
    Text,
    Image,
}

impl NodeContext {
    pub fn text(text: &str, font_system: &mut FontSystem) -> Self {
        NodeContext::Text
    }

    pub fn image(width: f32, height: f32) -> Self {
        NodeContext::Image
    }
}

pub fn measure_function(
    known_dimensions: taffy::Size<Option<f32>>,
    available_space: taffy::Size<taffy::AvailableSpace>,
    node_context: Option<&mut NodeContext>,
    font_system: &mut FontSystem,
) -> Size<f32> {
}
