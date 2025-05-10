use crate::{
    components::{Bounds, Component},
    config::{Config, StyleState},
    manager::UiState,
    rendering::texture_renderer::{self, TextureArea, TextureBounds},
    utils::buffers,
    utils::image_data::ImageData,
    Image,
};
use resvg::usvg;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::Path,
    rc::Rc,
    sync::{Arc, LazyLock, Mutex},
};

use super::Data;

static ICON_CACHE: LazyLock<Cache> = LazyLock::new(Cache::default);
type IconMap = BTreeMap<Box<Path>, ImageData>;

#[derive(Default)]
pub struct Cache(Mutex<IconMap>);

impl Cache {
    pub fn insert<P>(&self, icon_path: &P, data: ImageData)
    where
        P: AsRef<Path>,
    {
        let mut icon_map = self.0.lock().unwrap();
        let entry = icon_path.as_ref();

        icon_map.insert(entry.into(), data);
    }

    pub fn get<P>(&self, icon_path: P) -> Option<ImageData>
    where
        P: AsRef<Path>,
    {
        let theme_map = self.0.lock().unwrap();

        theme_map.get(icon_path.as_ref()).cloned()
    }
}

#[derive(Default)]
pub struct Icons {
    id: u32,
    icon: Option<ImageData>,
    app_icon: Option<ImageData>,
    x: f32,
    y: f32,
    ui_state: Rc<RefCell<UiState>>,
    config: Rc<Config>,
    app_name: Arc<str>,
}

impl Icons {
    pub fn new(
        id: u32,
        image: Option<&Image>,
        app_icon: Option<&str>,
        config: Rc<Config>,
        ui_state: Rc<RefCell<UiState>>,
        app_name: Arc<str>,
    ) -> Self {
        let icon = match image {
            Some(Image::Data(image_data)) => Some(
                image_data
                    .clone()
                    .to_rgba()
                    .resize(config.general.icon_size),
            ),
            Some(Image::File(file)) => get_icon(file, config.general.icon_size as u16),
            Some(Image::Name(name)) => find_icon(
                name,
                config.general.icon_size as u16,
                config.general.theme.as_ref(),
            ),
            _ => None,
        };

        let app_icon = app_icon.as_ref().and_then(|icon| {
            find_icon(
                icon,
                config.general.icon_size as u16,
                config.general.theme.as_deref().as_ref(),
            )
        });

        let (final_app_icon, final_icon) = match icon.is_some() {
            true => (app_icon, icon),
            false => (None, app_icon),
        };

        Self {
            id,
            icon: final_icon,
            app_icon: final_app_icon,
            x: 0.,
            y: 0.,
            ui_state,
            config,
            app_name,
        }
    }
}

impl Component for Icons {
    type Style = StyleState;

    fn get_config(&self) -> &Config {
        &self.config
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_app_name(&self) -> &str {
        &self.app_name
    }

    fn get_ui_state(&self) -> std::cell::Ref<'_, UiState> {
        self.ui_state.borrow()
    }

    fn get_style(&self) -> &Self::Style {
        self.get_notification_style()
    }

    fn get_bounds(&self) -> Bounds {
        let style = self.config.find_style(
            &self.app_name,
            self.ui_state.borrow().selected == Some(self.id),
        );

        let (width, height) = self
            .icon
            .as_ref()
            .map(|i| {
                (
                    i.width() as f32
                        + style.icon.padding.right
                        + style.icon.padding.left
                        + style.icon.margin.left
                        + style.icon.margin.right,
                    i.height() as f32
                        + style.icon.padding.top
                        + style.icon.padding.bottom
                        + style.icon.margin.top
                        + style.icon.margin.bottom,
                )
            })
            .unwrap_or((0., 0.));

        Bounds {
            x: self.x,
            y: self.y,
            width,
            height,
        }
    }

    fn get_render_bounds(&self) -> Bounds {
        let style = self.config.find_style(
            &self.app_name,
            self.ui_state.borrow().selected == Some(self.id),
        );

        let (width, height) = self
            .icon
            .as_ref()
            .map(|i| {
                (
                    i.width() as f32 + style.icon.padding.right + style.icon.padding.left,
                    i.height() as f32 + style.icon.padding.top + style.icon.padding.bottom,
                )
            })
            .unwrap_or((0., 0.));

        Bounds {
            x: self.x + style.icon.margin.left,
            y: self.y + style.icon.margin.top,
            width,
            height,
        }
    }

    fn get_instances(&self, _: &crate::Urgency) -> Vec<buffers::Instance> {
        Vec::new()
    }

    fn get_text_areas(&self, _: &crate::Urgency) -> Vec<glyphon::TextArea> {
        Vec::new()
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea> {
        let mut texture_areas = Vec::new();

        let style = self.config.find_style(
            &self.app_name,
            self.ui_state.borrow().selected == Some(self.id),
        );

        let mut bounds = self.get_render_bounds();

        if let Some(icon) = self.icon.as_ref() {
            texture_areas.push(TextureArea {
                left: bounds.x,
                top: bounds.y,
                width: bounds.width,
                height: bounds.height,
                scale: self.ui_state.borrow().scale,
                border_size: style.icon.border.size.into(),
                bounds: TextureBounds {
                    left: bounds.x as u32,
                    top: bounds.y as u32,
                    right: (bounds.x + bounds.width) as u32,
                    bottom: (bounds.y + bounds.height) as u32,
                },
                data: icon.data(),
                radius: style.icon.border.radius.into(),
                depth: 0.9,
            });

            bounds.x += bounds.height - self.config.general.app_icon_size as f32;
            bounds.y += bounds.height - self.config.general.app_icon_size as f32;
        }

        if let Some(app_icon) = self.app_icon.as_ref() {
            let app_icon_size = self.config.general.app_icon_size as f32;

            texture_areas.push(TextureArea {
                left: bounds.x,
                top: bounds.y,
                width: app_icon_size,
                height: app_icon_size,
                scale: self.ui_state.borrow().scale,
                border_size: style.icon.border.size.into(),
                bounds: TextureBounds {
                    left: bounds.x as u32,
                    top: bounds.y as u32,
                    right: (bounds.x + app_icon_size) as u32,
                    bottom: (bounds.y + app_icon_size) as u32,
                },
                data: app_icon.data(),
                radius: style.app_icon.border.radius.into(),
                depth: 0.8,
            });
        }

        texture_areas
    }

    fn get_data(&self, _: &crate::Urgency) -> Vec<Data> {
        self.get_textures().into_iter().map(Data::Texture).collect()
    }
}

fn find_icon<T>(name: T, icon_size: u16, theme: Option<T>) -> Option<ImageData>
where
    T: AsRef<str>,
{
    let icon_path = freedesktop_icons::lookup(name.as_ref())
        .with_size(icon_size)
        .with_theme(theme.as_ref().map(AsRef::as_ref).unwrap_or("hicolor"))
        .force_svg()
        .with_cache()
        .find()?;

    get_icon(&icon_path, icon_size)
}

pub fn get_icon<T>(icon_path: T, icon_size: u16) -> Option<ImageData>
where
    T: AsRef<Path>,
{
    if let Some(icon) = ICON_CACHE.get(icon_path.as_ref()) {
        return Some(icon);
    }

    let image = if icon_path
        .as_ref()
        .extension()
        .is_some_and(|extension| extension == "svg")
    {
        let tree = {
            let opt = usvg::Options {
                resources_dir: Some(icon_path.as_ref().to_path_buf()),
                ..usvg::Options::default()
            };

            let svg_data = std::fs::read(icon_path.as_ref()).ok()?;
            usvg::Tree::from_data(&svg_data, &opt).ok()?
        };

        let mut pixmap = tiny_skia::Pixmap::new(icon_size as u32, icon_size as u32)?;

        let scale_x = icon_size as f32 / tree.size().width();
        let scale_y = icon_size as f32 / tree.size().height();

        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale_x, scale_y),
            &mut pixmap.as_mut(),
        );

        image::load_from_memory(&pixmap.encode_png().ok()?)
    } else {
        image::open(icon_path.as_ref())
    };

    let image_data = ImageData::try_from(image.ok()?);
    let image_data = image_data
        .ok()
        .map(|i| i.to_rgba().resize(icon_size as u32))?;
    ICON_CACHE.insert(&icon_path, image_data.clone());
    Some(image_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbaImage};
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::sync::Arc;

    #[test]
    fn cache_insert_and_retrieve() {
        let cache = Cache::default();
        let path = PathBuf::from("test_icon.png");

        let img = RgbaImage::new(32, 32);
        let data = ImageData::try_from(DynamicImage::ImageRgba8(img)).unwrap();

        cache.insert(&path, data.clone());
        assert_eq!(cache.get(&path).unwrap(), data);
    }

    #[test]
    fn new_with_image_data() {
        let config = Rc::new(Config::default());
        let ui_state = Rc::new(RefCell::new(UiState::default()));

        let img = RgbaImage::new(64, 64);
        let image_data = ImageData::try_from(DynamicImage::ImageRgba8(img)).unwrap();

        let image = Image::Data(image_data.clone());
        let icons = Icons::new(1, Some(&image), None, config, ui_state, Arc::from("app"));

        assert!(icons.icon.is_some());
        assert_eq!(icons.icon.unwrap().width(), 64);
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = Cache::default();
        let non_existent_path = Path::new("non_existent.png");
        assert!(cache.get(non_existent_path).is_none());
    }

    #[test]
    fn set_position_updates_coordinates() {
        let mut icons = Icons::default();
        icons.set_position(100., 200.);
        assert_eq!(icons.x, 100.);
        assert_eq!(icons.y, 200.);
    }
}
