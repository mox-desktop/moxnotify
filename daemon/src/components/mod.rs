pub mod button;
pub mod icons;
pub mod notification;
pub mod progress;
pub mod text;

use std::sync::atomic::Ordering;

use crate::{
    Urgency,
    config::{Config, StyleState},
    manager::UiState,
    rendering::texture_renderer,
    utils::buffers,
};

pub enum Data<'a> {
    Instance(buffers::Instance),
    TextArea(glyphon::TextArea<'a>),
    Texture(texture_renderer::TextureArea<'a>),
}

#[derive(Default, Debug)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub trait Component {
    type Style;

    fn get_config(&self) -> &Config;

    fn get_app_name(&self) -> &str;

    fn get_id(&self) -> u32;

    fn get_ui_state(&self) -> &UiState;

    fn get_notification_style(&self) -> &StyleState {
        self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected.load(Ordering::Relaxed)
                && self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id(),
        )
    }

    fn get_style(&self) -> &Self::Style;

    fn get_instances(&self, urgency: &Urgency) -> Vec<buffers::Instance>;

    fn get_text_areas(&self, urgency: &Urgency) -> Vec<glyphon::TextArea<'_>>;

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea<'_>>;

    fn get_bounds(&self) -> Bounds;

    fn get_render_bounds(&self) -> Bounds;

    fn set_position(&mut self, x: f32, y: f32);

    fn get_data(&self, urgency: &Urgency) -> Vec<Data<'_>> {
        self.get_instances(urgency)
            .into_iter()
            .map(Data::Instance)
            .chain(self.get_text_areas(urgency).into_iter().map(Data::TextArea))
            .collect()
    }
}
