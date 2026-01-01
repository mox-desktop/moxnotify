pub mod body;
pub mod markup;
pub mod summary;

use super::Component;
use glyphon::FontSystem;

pub trait Text: Component {
    fn set_size(&mut self, font_system: &mut FontSystem, width: Option<f32>, height: Option<f32>);

    fn set_text<T>(&mut self, font_system: &mut FontSystem, text: T)
    where
        T: AsRef<str>;
}
