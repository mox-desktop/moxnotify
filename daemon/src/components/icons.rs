use super::Data;
use crate::{
    Image,
    components::{self, Component},
    config,
    rendering::texture_renderer::{self, TextureArea, TextureBounds},
    utils::{buffers, image_data::ImageData, taffy::GlobalLayout},
};
use resvg::usvg;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{LazyLock, Mutex, atomic::Ordering},
};
use taffy::style_helpers::{auto, length, line, span};

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

pub struct Icons {
    node: taffy::NodeId,
    icon: Option<ImageData>,
    app_icon: Option<ImageData>,
    x: f32,
    y: f32,
    context: components::Context,
}

impl Icons {
    #[must_use]
    pub fn new(
        tree: &mut taffy::TaffyTree<()>,
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

        let (final_app_icon, final_icon) = if icon.is_some() {
            (app_icon, icon)
        } else {
            (None, app_icon)
        };

        let node = tree.new_leaf(taffy::Style::DEFAULT).unwrap();

        Self {
            node,
            context,
            icon: final_icon,
            app_icon: final_app_icon,
            x: 0.,
            y: 0.,
        }
    }
}

impl Component for Icons {
    type Style = config::Icon;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        &self.get_notification_style().icon
    }

    fn get_instances(&self, _: &taffy::TaffyTree<()>, _: crate::Urgency) -> Vec<buffers::Instance> {
        Vec::new()
    }

    fn get_text_areas(
        &self,
        _: &taffy::TaffyTree<()>,
        _: crate::Urgency,
    ) -> Vec<glyphon::TextArea<'_>> {
        Vec::new()
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        let style = self.get_style();

        let (width, height) = self
            .icon
            .as_ref()
            .map_or((0., 0.), |i| (i.width() as f32, i.height() as f32));

        self.node = tree
            .new_leaf(taffy::Style {
                grid_row: span(2),
                grid_column: line(1),
                size: taffy::Size {
                    width: length(width),
                    height: length(height),
                },
                padding: taffy::Rect {
                    top: length(style.padding.top.resolve(0.)),
                    left: length(style.padding.left.resolve(0.)),
                    right: length(style.padding.right.resolve(0.)),
                    bottom: length(style.padding.bottom.resolve(0.)),
                },
                margin: taffy::Rect {
                    left: if style.margin.left.is_auto() {
                        auto()
                    } else {
                        length(style.margin.left.resolve(0.))
                    },
                    right: if style.margin.right.is_auto() {
                        auto()
                    } else {
                        length(style.margin.right.resolve(0.))
                    },
                    top: if style.margin.top.is_auto() {
                        auto()
                    } else {
                        length(style.margin.top.resolve(0.))
                    },
                    bottom: if style.margin.bottom.is_auto() {
                        auto()
                    } else {
                        length(style.margin.bottom.resolve(0.))
                    },
                },
                border: taffy::Rect {
                    left: length(style.border.size.left.resolve(0.)),
                    right: length(style.border.size.left.resolve(0.)),
                    top: length(style.border.size.left.resolve(0.)),
                    bottom: length(style.border.size.left.resolve(0.)),
                },
                ..Default::default()
            })
            .unwrap();
    }

    fn apply_computed_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
    }

    fn get_textures(&self, tree: &taffy::TaffyTree<()>) -> Vec<texture_renderer::TextureArea<'_>> {
        let mut texture_areas = Vec::new();

        let style = self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id()
                && self.get_ui_state().selected.load(Ordering::Relaxed),
        );

        let mut bounds = self.get_render_bounds(tree);

        if let Some(icon) = self.icon.as_ref() {
            texture_areas.push(TextureArea {
                left: bounds.x,
                top: bounds.y,
                width: bounds.width,
                height: bounds.height,
                scale: self.get_ui_state().scale.load(Ordering::Relaxed),
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

            bounds.x += bounds.height - self.get_config().general.app_icon_size as f32;
            bounds.y += bounds.height - self.get_config().general.app_icon_size as f32;
        }

        if let Some(app_icon) = self.app_icon.as_ref() {
            let app_icon_size = self.get_config().general.app_icon_size as f32;

            texture_areas.push(TextureArea {
                left: bounds.x,
                top: bounds.y,
                width: app_icon_size,
                height: app_icon_size,
                scale: self.get_ui_state().scale.load(Ordering::Relaxed),
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

    fn get_data(&self, tree: &taffy::TaffyTree<()>, _: crate::Urgency) -> Vec<Data<'_>> {
        self.get_textures(tree)
            .into_iter()
            .map(Data::Texture)
            .collect()
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
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
    if let Ok(image_data) = image_data.as_ref() {
        ICON_CACHE.insert(&icon_path, image_data.clone());
    }
    image_data.ok()
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
