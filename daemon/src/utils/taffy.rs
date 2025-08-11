use taffy::{
    TaffyTree,
    style_helpers::{auto, fr, length, line, max_content, span},
};

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
