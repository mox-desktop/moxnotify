use super::{Button, Component, Hint, State};
use crate::{
    components::{self, text::body::Anchor},
    config::button::ButtonState,
    rendering::{text_renderer::TextContext, texture_renderer},
    utils::{
        buffers,
        taffy::{GlobalLayout, NodeContext},
    },
};
use std::sync::Arc;

pub struct AnchorButton {
    pub node: taffy::NodeId,
    pub context: components::Context,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub text: TextContext,
    pub state: State,
    pub tx: Option<calloop::channel::Sender<crate::Event>>,
    pub anchor: Arc<Anchor>,
}

impl Component for AnchorButton {
    type Style = ButtonState;

    fn get_context(&self) -> &crate::components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        &self.context.config.styles.hover.buttons.dismiss.default
    }

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
        urgency: crate::Urgency,
    ) -> Vec<buffers::Instance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds(tree);
        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.color(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.color(urgency),
            scale: 0.,
            depth: 0.8,
        }]
    }

    fn get_text_areas(
        &self,
        _: &taffy::TaffyTree<NodeContext>,
        urgency: crate::Urgency,
    ) -> Vec<glyphon::TextArea<'_>> {
        let style = self.get_style();
        vec![glyphon::TextArea {
            buffer: &self.text.buffer,
            left: 0.,
            top: 0.,
            scale: 0.,
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }]
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<NodeContext>) {
        self.hint.update_layout(tree);
        self.node = tree.new_leaf(taffy::Style::DEFAULT).unwrap();
    }

    fn apply_computed_layout(&mut self, tree: &taffy::TaffyTree<NodeContext>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
    }

    fn get_textures(
        &self,
        _: &taffy::TaffyTree<NodeContext>,
    ) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
    }
}

impl Button for AnchorButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        if let Some(tx) = self.tx.as_ref() {
            _ = tx.send(crate::Event::InvokeAnchor(Arc::clone(&self.anchor.href)));
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn button_type(&self) -> super::ButtonType {
        super::ButtonType::Anchor
    }

    fn state(&self) -> State {
        self.state
    }

    fn hover(&mut self) {
        self.state = State::Hovered;
    }

    fn unhover(&mut self) {
        self.state = State::Unhovered;
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}
