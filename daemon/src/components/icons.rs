use super::Data;
use crate::{
    Image,
    components::{self, Component},
    config,
    utils::{
        image_data::ImageContext,
        taffy::{GlobalLayout, NodeContext},
    },
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
use taffy::style_helpers::{auto, length, line, span};

static ICON_CACHE: LazyLock<Cache> = LazyLock::new(Cache::default);
type IconMap = BTreeMap<Box<Path>, ImageContext>;

#[derive(Default)]
pub struct Cache(Mutex<IconMap>);

impl Cache {
    pub fn insert<P>(&self, icon_path: &P, data: ImageContext)
    where
        P: AsRef<Path>,
    {
        let mut icon_map = self.0.lock().unwrap();
        let entry = icon_path.as_ref();

        icon_map.insert(entry.into(), data);
    }

    pub fn get<P>(&self, icon_path: P) -> Option<ImageContext>
    where
        P: AsRef<Path>,
    {
        let theme_map = self.0.lock().unwrap();

        theme_map.get(icon_path.as_ref()).cloned()
    }
}

pub struct Icons {
    node: taffy::NodeId,
    icon: Option<ImageContext>,
    app_icon: Option<ImageContext>,
    x: f32,
    y: f32,
    context: components::Context,
}

impl Icons {
    #[must_use]
    pub fn new(
        tree: &mut taffy::TaffyTree<NodeContext>,
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

    fn get_instances(
        &self,
        _: &taffy::TaffyTree<NodeContext>,
        _: crate::Urgency,
    ) -> Vec<shape_renderer::ShapeInstance> {
        Vec::new()
    }

    fn get_text_areas(
        &self,
        _: &taffy::TaffyTree<NodeContext>,
        _: crate::Urgency,
    ) -> Vec<glyphon::TextArea<'_>> {
        Vec::new()
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<NodeContext>) {
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

    fn apply_computed_layout(&mut self, tree: &taffy::TaffyTree<NodeContext>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
    }

    fn get_textures(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
    ) -> Vec<texture_renderer::TextureArea<'_>> {
        let mut texture_areas = Vec::new();

        let style = self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id()
                && self.get_ui_state().selected.load(Ordering::Relaxed),
        );

        let mut layout = tree.global_layout(self.get_node_id()).unwrap();
        if let Some(icon) = self.icon.as_ref() {
            let mut buffer = Buffer::new(icon.width() as f32, icon.height() as f32);
            buffer.set_bytes(icon.data());

            texture_areas.push(TextureArea {
                left: layout.location.x,
                top: layout.location.y,
                scale: self.get_ui_state().scale.load(Ordering::Relaxed),
                rotation: 0.0,
                bounds: TextureBounds {
                    left: layout.location.x as u32,
                    top: layout.location.y as u32,
                    right: (layout.location.x + layout.content_box_width()) as u32,
                    bottom: (layout.location.y + layout.content_box_height()) as u32,
                },
                skew: [0.0, 0.0],
                radius: style.icon.border.radius.into(),
                buffer,
                depth: 0.9,
            });

            layout.location.x +=
                layout.content_box_width() - self.get_config().general.app_icon_size as f32;
            layout.location.y +=
                layout.content_box_height() - self.get_config().general.app_icon_size as f32;
        }

        if let Some(app_icon) = self.app_icon.as_ref() {
            let app_icon_size = self.get_config().general.app_icon_size as f32;

            texture_areas.push(TextureArea {
                left: layout.location.x,
                top: layout.location.y,
                scale: self.get_ui_state().scale.load(Ordering::Relaxed),
                rotation: 0.0,
                bounds: TextureBounds {
                    left: layout.location.x as u32,
                    top: layout.location.y as u32,
                    right: (layout.location.x + app_icon_size) as u32,
                    bottom: (layout.location.y + app_icon_size) as u32,
                },
                skew: [0.0, 0.0],
                radius: style.app_icon.border.radius.into(),
                buffer: {
                    let mut buffer = Buffer::new(app_icon_size, app_icon_size);
                    buffer.set_bytes(app_icon.data());
                    buffer
                },
                depth: 0.8,
            });
        }

        texture_areas
    }

    fn get_data(&self, tree: &taffy::TaffyTree<NodeContext>, _: crate::Urgency) -> Vec<Data<'_>> {
        self.get_textures(tree)
            .into_iter()
            .map(Data::Texture)
            .collect()
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
    }
}

fn find_icon<T>(name: T, icon_size: u16, theme: Option<T>) -> Option<ImageContext>
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

pub fn get_icon<T>(icon_path: T, icon_size: u16) -> Option<ImageContext>
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

        ImageContext::try_from(image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(
            icon_size as u32,
            icon_size as u32,
            data,
        )?))
        .ok()
    } else {
        let image = image::open(icon_path.as_ref()).ok()?;
        ImageContext::try_from(image).ok()
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
