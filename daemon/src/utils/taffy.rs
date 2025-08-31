use crate::{config::Font, rendering::text_renderer::TextContext};
use glyphon::FontSystem;
use taffy::{Size, TaffyTree};

pub trait GlobalLayout {
    fn global_layout(&self, node: taffy::NodeId) -> taffy::TaffyResult<taffy::Layout>;
}

impl<T> GlobalLayout for TaffyTree<T> {
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
    Text(TextContext),
    //Image(ImageContext),
}

impl NodeContext {
    pub fn text<T>(font: &Font, font_system: &mut FontSystem, body: T) -> Self
    where
        T: AsRef<str>,
    {
        let text = TextContext::new(font, font_system, body);
        NodeContext::Text(text)
    }

    //pub fn image(width: f32, height: f32) -> Self {
    //    NodeContext::Image(ImageContext::)
    //}
}

pub fn measure_function(
    known_dimensions: taffy::Size<Option<f32>>,
    available_space: taffy::Size<taffy::AvailableSpace>,
    node_context: Option<&mut NodeContext>,
    font_system: &mut FontSystem,
) -> Size<f32> {
    if let Size {
        width: Some(width),
        height: Some(height),
    } = known_dimensions
    {
        return Size { width, height };
    }

    match node_context {
        None => Size::ZERO,
        Some(NodeContext::Text(text_context)) => {
            text_context.measure(known_dimensions, available_space, font_system)
        } //Some(NodeContext::Image(image_context)) => {
          //    image_measure_function(known_dimensions, image_context)
          //}
    }
}
