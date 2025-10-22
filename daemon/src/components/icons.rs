use crate::{
    Image,
    components::{self, Bounds, Component},
    config::StyleState,
    utils::image_data::ImageData,
};
use moxui::{
    shape_renderer,
    texture_renderer::{self, Buffer, TextureArea, TextureBounds},
};
use resvg::usvg;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{LazyLock, Mutex, atomic::Ordering},
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
    icon: Option<ImageData>,
    app_icon: Option<ImageData>,
    x: f32,
    y: f32,
    context: components::Context,
}

impl Icons {
    pub fn new(
        context: components::Context,
        image: Option<&Image>,
        app_icon: Option<&str>,
    ) -> Self {
        let icon = match image {
            Some(Image::Data(image_data)) => image_data
                .clone()
                .to_rgba()
                .resize(context.config.general.icon_size)
                .ok(),
            Some(Image::File(file)) => get_icon(file, context.config.general.icon_size as u16),
            Some(Image::Name(name)) => find_icon(
                name,
                context.config.general.icon_size as u16,
                context.config.general.theme.as_ref(),
            ),
            _ => None,
        };

        let app_icon = app_icon.as_ref().and_then(|icon| {
            find_icon(
                icon,
                context.config.general.icon_size as u16,
                context.config.general.theme.as_deref().as_ref(),
            )
        });

        let (final_app_icon, final_icon) = match icon.is_some() {
            true => (app_icon, icon),
            false => (None, app_icon),
        };

        Self {
            context,
            icon: final_icon,
            app_icon: final_app_icon,
            x: 0.,
            y: 0.,
        }
    }
}

impl Component for Icons {
    type Style = StyleState;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        self.get_notification_style()
    }

    fn get_bounds(&self) -> Bounds {
        let style = self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id()
                && self.get_ui_state().selected.load(Ordering::Relaxed),
        );

        let (width, height) = self.icon.as_ref().map_or((0., 0.), |i| {
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
        });

        Bounds {
            x: self.x,
            y: self.y,
            width,
            height,
        }
    }

    fn get_render_bounds(&self) -> Bounds {
        let style = self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id()
                && self.get_ui_state().selected.load(Ordering::Relaxed),
        );

        let (width, height) = self.icon.as_ref().map_or((0., 0.), |i| {
            (
                i.width() as f32 + style.icon.padding.right + style.icon.padding.left,
                i.height() as f32 + style.icon.padding.top + style.icon.padding.bottom,
            )
        });

        Bounds {
            x: self.x + style.icon.margin.left,
            y: self.y + style.icon.margin.top,
            width,
            height,
        }
    }

    fn get_instances(&self, _: crate::Urgency) -> Vec<shape_renderer::ShapeInstance> {
        Vec::new()
    }

    fn get_text_areas(&self, _: crate::Urgency) -> Vec<glyphon::TextArea<'_>> {
        Vec::new()
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea<'_>> {
        let mut texture_areas = Vec::new();

        let style = self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id()
                && self.get_ui_state().selected.load(Ordering::Relaxed),
        );

        let mut bounds = self.get_render_bounds();

        if let Some(icon) = self.icon.as_ref() {
            let mut buffer = Buffer::new(icon.width() as f32, icon.height() as f32);
            buffer.set_bytes(icon.data());

            texture_areas.push(TextureArea {
                left: bounds.x + style.icon.padding.left,
                top: bounds.y + style.icon.padding.top,
                scale: 1.0,
                rotation: 0.,
                bounds: TextureBounds {
                    left: bounds.x as u32,
                    top: bounds.y as u32,
                    right: (bounds.x + bounds.width) as u32,
                    bottom: (bounds.y + bounds.height) as u32,
                },
                skew: [0., 0.],
                radius: style.icon.border.radius.into(),
                buffer,
                depth: 0.9,
            });

            bounds.x += bounds.height - self.get_config().general.app_icon_size as f32;
            bounds.y += bounds.height - self.get_config().general.app_icon_size as f32;
        }

        if let Some(app_icon) = self.app_icon.as_ref() {
            let app_icon_size = self.get_config().general.app_icon_size as f32;
            texture_areas.push(TextureArea::simple(
                app_icon.data(),
                bounds.x + style.icon.padding.left,
                bounds.y + style.icon.padding.top,
                app_icon.width() as f32,
                app_icon.height() as f32,
                TextureBounds {
                    left: bounds.x as u32,
                    top: bounds.y as u32,
                    right: (bounds.x
                        + app_icon_size
                        + style.icon.padding.left
                        + style.icon.padding.right) as u32,
                    bottom: (bounds.y
                        + app_icon_size
                        + style.icon.padding.top
                        + style.icon.padding.bottom) as u32,
                },
                style.app_icon.border.radius.into(),
                style.icon.border.size.into(),
                0.8,
            ));
        }

        texture_areas
    }

    fn get_data(&self, _: crate::Urgency) -> Vec<Data<'_>> {
        self.get_textures().into_iter().map(Data::Texture).collect()
    }
}

fn find_icon<T>(name: T, icon_size: u16, theme: Option<T>) -> Option<ImageData>
where
    T: AsRef<str>,
{
    let icon_path = freedesktop_icons::lookup(name.as_ref())
        .with_size(icon_size)
        .with_theme(theme.as_ref().map_or("hicolor", AsRef::as_ref))
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

    let image_data = if icon_path
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
        pixmap.fill(tiny_skia::Color::TRANSPARENT);

        let scale_x = icon_size as f32 / tree.size().width();
        let scale_y = icon_size as f32 / tree.size().height();

        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale_x, scale_y),
            &mut pixmap.as_mut(),
        );

        let mut data = pixmap.take();
        data.chunks_exact_mut(4).for_each(|pixel| {
            let alpha = pixel[3] as f32 / 255.0;
            if alpha > 0.0 && alpha < 1.0 {
                pixel[0] = ((pixel[0] as f32 / alpha).min(255.0)) as u8;
                pixel[1] = ((pixel[1] as f32 / alpha).min(255.0)) as u8;
                pixel[2] = ((pixel[2] as f32 / alpha).min(255.0)) as u8;
            }
        });

        ImageData::try_from(image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(
            icon_size as u32,
            icon_size as u32,
            data,
        )?))
        .ok()
    } else {
        let image = image::open(icon_path.as_ref()).ok()?;
        ImageData::try_from(image).ok()
    };

    let image_data = if icon_path
        .as_ref()
        .extension()
        .is_some_and(|ext| ext == "svg")
    {
        image_data.map(|i| i.to_rgba())
    } else {
        image_data.and_then(|i| i.to_rgba().resize(icon_size as u32).ok())
    };

    if let Some(ref data) = image_data {
        ICON_CACHE.insert(&icon_path, data.clone());
    }
    image_data
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbaImage};
    use std::path::{Path, PathBuf};

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
        let img = RgbaImage::new(64, 64);
        let image_data = ImageData::try_from(DynamicImage::ImageRgba8(img)).unwrap();

        let image = Image::Data(image_data.clone());
        let context = components::Context {
            id: 1,
            config: crate::Config::default().into(),
            ui_state: crate::manager::UiState::default(),
            app_name: "app".into(),
        };
        let icons = Icons::new(context, Some(&image), None);

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
