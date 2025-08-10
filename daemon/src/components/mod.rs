pub mod button;
pub mod icons;
pub mod notification;
pub mod progress;
pub mod text;

use crate::{
    Urgency,
    config::{Config, StyleState},
    manager::UiState,
    utils::taffy::{GlobalLayout, NodeContext},
};
use moxui::{shape_renderer, texture_renderer};
use std::sync::{Arc, atomic::Ordering};

#[derive(Clone, Default)]
pub struct Context {
    pub id: u32,
    pub app_name: Arc<str>,
    pub config: Arc<Config>,
    pub ui_state: UiState,
}

pub enum Data<'a> {
    Instance(shape_renderer::ShapeInstance),
    TextArea(glyphon::TextArea<'a>),
    Texture(texture_renderer::TextureArea<'a>),
}

#[derive(Default, Debug, Clone)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub trait Component {
    type Style;

    fn get_context(&self) -> &Context;

    fn get_config(&self) -> &Config {
        &self.get_context().config
    }

    fn get_app_name(&self) -> &str {
        &self.get_context().app_name
    }

    fn get_id(&self) -> u32 {
        self.get_context().id
    }

    fn get_ui_state(&self) -> &UiState {
        &self.get_context().ui_state
    }

    fn get_notification_style(&self) -> &StyleState {
        self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected.load(Ordering::Relaxed)
                && self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id(),
        )
    }

    fn get_style(&self) -> &Self::Style;

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
        urgency: Urgency,
    ) -> Vec<shape_renderer::ShapeInstance>;

    fn get_text_areas(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
        urgency: Urgency,
    ) -> Vec<glyphon::TextArea<'_>>;

    fn get_textures(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
    ) -> Vec<texture_renderer::TextureArea<'_>>;

    fn get_bounds(&self, tree: &taffy::TaffyTree<NodeContext>) -> Bounds {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        let style = tree.style(self.get_node_id()).unwrap();

        Bounds {
            x: layout.location.x - style.margin.left.into_raw().value(),
            y: layout.location.y - style.margin.top.into_raw().value(),
            width: layout.size.width
                + style.margin.left.into_raw().value()
                + style.margin.right.into_raw().value(),
            height: layout.size.height
                + style.margin.top.into_raw().value()
                + style.margin.bottom.into_raw().value(),
        }
    }

    fn get_render_bounds(&self, tree: &taffy::TaffyTree<NodeContext>) -> Bounds {
        let layout = tree.global_layout(self.get_node_id()).unwrap();

        Bounds {
            x: layout.location.x,
            y: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        }
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<NodeContext>);

    fn apply_computed_layout(&mut self, tree: &taffy::TaffyTree<NodeContext>);

    fn get_data(&self, tree: &taffy::TaffyTree<NodeContext>, urgency: Urgency) -> Vec<Data<'_>> {
        self.get_instances(tree, urgency)
            .into_iter()
            .map(Data::Instance)
            .chain(
                self.get_text_areas(tree, urgency)
                    .into_iter()
                    .map(Data::TextArea),
            )
            .collect()
    }

    fn get_node_id(&self) -> taffy::NodeId;
}
