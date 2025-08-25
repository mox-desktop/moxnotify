mod action;
mod anchor;
mod dismiss;

use super::text::body;
use crate::{
    Urgency,
    components::{self, Component, Data},
    config::{
        self,
        button::ButtonState,
        keymaps::{self},
    },
    rendering::{text_renderer, texture_renderer},
    utils::{buffers, taffy::GlobalLayout},
};
use action::ActionButton;
use anchor::AnchorButton;
use dismiss::DismissButton;
use glyphon::{FontSystem, TextArea};
use std::sync::{Arc, atomic::Ordering};
use taffy::style_helpers::auto;

#[derive(Clone, Copy, Debug)]
pub enum State {
    Unhovered,
    Hovered,
}

pub trait Button: Component + Send + Sync {
    fn hint(&self) -> &Hint;

    fn click(&self);

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    fn button_type(&self) -> ButtonType;

    fn state(&self) -> State;

    fn hover(&mut self);

    fn unhover(&mut self);

    fn set_hint(&mut self, hint: Hint);
}

#[derive(Clone, PartialEq)]
pub enum ButtonType {
    Dismiss,
    Action,
    Anchor,
}

pub struct NotReady;
pub struct Ready;
pub struct Finished;

pub struct ButtonManager<State = NotReady> {
    context: components::Context,
    pub action_container: Option<taffy::NodeId>,
    buttons: Vec<Box<dyn Button<Style = ButtonState>>>,
    urgency: Urgency,
    sender: Option<calloop::channel::Sender<crate::Event>>,
    _state: std::marker::PhantomData<State>,
}

impl ButtonManager<NotReady> {
    pub fn new(
        context: components::Context,
        urgency: Urgency,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) -> Self {
        Self {
            action_container: None,
            context,
            buttons: Vec::new(),
            urgency,
            sender,
            _state: std::marker::PhantomData,
        }
    }

    pub fn add_actions(
        self,
        tree: &mut taffy::TaffyTree<()>,
        actions: &[(Arc<str>, Arc<str>)],
        font_system: &mut FontSystem,
    ) -> Self {
        self.internal_add_actions(tree, actions, font_system)
    }

    pub fn add_anchors(
        self,
        tree: &mut taffy::TaffyTree,
        anchors: &[Arc<body::Anchor>],
        font_system: &mut FontSystem,
    ) -> Self {
        self.internal_add_anchors(tree, anchors, font_system)
    }

    pub fn add_dismiss(
        mut self,
        tree: &mut taffy::TaffyTree<()>,
        font_system: &mut FontSystem,
    ) -> ButtonManager<Ready> {
        let font = &self
            .context
            .config
            .styles
            .default
            .buttons
            .dismiss
            .default
            .font;
        let text = text_renderer::Text::new(font, font_system, "X");

        let button = DismissButton {
            node: tree.new_leaf(taffy::Style::DEFAULT).unwrap(),
            hint: Hint::new(tree, self.context.clone(), "", font_system),
            text,
            x: 0.,
            y: 0.,
            state: State::Unhovered,
            tx: self.sender.clone(),
            context: self.context.clone(),
        };

        self.buttons.push(Box::new(button));

        ButtonManager {
            context: self.context,
            buttons: self.buttons,
            urgency: self.urgency,
            sender: self.sender,
            action_container: self.action_container,
            _state: std::marker::PhantomData,
        }
    }
}

impl ButtonManager<Ready> {
    pub fn add_actions(
        self,
        tree: &mut taffy::TaffyTree<()>,
        actions: &[(Arc<str>, Arc<str>)],
        font_system: &mut FontSystem,
    ) -> Self {
        self.internal_add_actions(tree, actions, font_system)
    }

    pub fn add_anchors(
        self,
        tree: &mut taffy::TaffyTree<()>,
        anchors: &[Arc<body::Anchor>],
        font_system: &mut FontSystem,
    ) -> Self {
        self.internal_add_anchors(tree, anchors, font_system)
    }

    pub fn finish(
        mut self,
        tree: &mut taffy::TaffyTree<()>,
        font_system: &mut FontSystem,
    ) -> ButtonManager<Finished> {
        let hint_chars: Vec<char> = self
            .context
            .config
            .general
            .hint_characters
            .chars()
            .collect();
        let n = hint_chars.len() as i32;

        self.buttons.iter_mut().enumerate().for_each(|(i, button)| {
            let mut m = i as i32;
            let mut indices = Vec::new();

            loop {
                let rem = (m % n) as usize;
                indices.push(rem);
                m = (m / n) - 1;
                if m < 0 {
                    break;
                }
            }

            indices.reverse();
            let combination: String = indices.into_iter().map(|i| hint_chars[i]).collect();
            let hint = Hint::new(tree, self.context.clone(), &combination, font_system);

            button.set_hint(hint);
        });

        ButtonManager {
            buttons: self.buttons,
            urgency: self.urgency,
            sender: self.sender,
            context: self.context,
            action_container: self.action_container,
            _state: std::marker::PhantomData,
        }
    }
}

impl ButtonManager<Finished> {
    #[must_use]
    pub fn click(&self, tree: &taffy::TaffyTree<()>, x: f64, y: f64) -> bool {
        self.buttons
            .iter()
            .find_map(|button| {
                let bounds = button.get_render_bounds(tree);
                if x >= bounds.x as f64
                    && y >= bounds.y as f64
                    && x <= (bounds.x + bounds.width) as f64
                    && y <= (bounds.y + bounds.height) as f64
                {
                    button.click();
                    Some(true)
                } else {
                    None
                }
            })
            .is_some()
    }

    pub fn hover(&mut self, tree: &taffy::TaffyTree<()>, x: f64, y: f64) -> bool {
        self.buttons
            .iter_mut()
            .find_map(|button| {
                let bounds = button.get_render_bounds(tree);
                if x >= bounds.x as f64
                    && y >= bounds.y as f64
                    && x <= (bounds.x + bounds.width) as f64
                    && y <= (bounds.y + bounds.height) as f64
                {
                    button.hover();
                    Some(true)
                } else {
                    button.unhover();
                    None
                }
            })
            .is_some()
    }

    pub fn hint<T>(&mut self, combination: T)
    where
        T: AsRef<str>,
    {
        if let Some(button) = self
            .buttons
            .iter()
            .find(|button| &*button.hint().combination == combination.as_ref())
        {
            button.click();
        }
    }

    #[must_use]
    pub fn instances(&self, tree: &taffy::TaffyTree<()>) -> Vec<buffers::Instance> {
        let mut buttons = self
            .buttons
            .iter()
            .flat_map(|button| button.get_instances(tree, self.urgency))
            .collect::<Vec<_>>();

        if self.context.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.context.ui_state.selected_id.load(Ordering::Relaxed) == self.context.id
            && self.context.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_instances(tree, self.urgency))
                .collect::<Vec<_>>();
            buttons.extend_from_slice(&hints);
        }

        buttons
    }

    #[must_use]
    pub fn text_areas(&self, tree: &taffy::TaffyTree<()>) -> Vec<TextArea<'_>> {
        let mut text_areas = self
            .buttons
            .iter()
            .flat_map(|button| button.get_text_areas(tree, self.urgency))
            .collect::<Vec<_>>();

        if self.context.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.context.ui_state.selected_id.load(Ordering::Relaxed) == self.context.id
            && self.context.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_text_areas(tree, self.urgency));
            text_areas.extend(hints);
        }

        text_areas
    }

    #[must_use]
    pub fn get_data(&self, tree: &taffy::TaffyTree<()>) -> Vec<Data<'_>> {
        let mut data = self
            .buttons
            .iter()
            .flat_map(|button| button.get_data(tree, self.urgency))
            .collect::<Vec<_>>();

        if self.context.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.context.ui_state.selected_id.load(Ordering::Relaxed) == self.context.id
            && self.context.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_data(tree, self.urgency));
            data.extend(hints);
        }

        data
    }

    pub fn set_action_widths(&mut self, width: f32) {
        self.buttons
            .iter_mut()
            .filter_map(|button| button.as_any_mut().downcast_mut::<ActionButton>())
            .for_each(|action| {
                action.width = width;
            });
    }
}

impl<S> ButtonManager<S> {
    fn internal_add_anchors(
        mut self,
        tree: &mut taffy::TaffyTree,
        anchors: &[Arc<body::Anchor>],
        font_system: &mut FontSystem,
    ) -> Self {
        if anchors.is_empty() {
            return self;
        }

        let font = &self
            .context
            .config
            .styles
            .default
            .buttons
            .action
            .default
            .font;

        self.buttons.extend(anchors.iter().map(|anchor| {
            let text = text_renderer::Text::new(font, font_system, "");
            Box::new(AnchorButton {
                node: tree.new_leaf(taffy::Style::DEFAULT).unwrap(),
                context: self.context.clone(),
                x: 0.,
                y: 0.,
                hint: Hint::new(tree, self.context.clone(), "", font_system),
                state: State::Unhovered,
                tx: self.sender.clone(),
                text,
                anchor: Arc::clone(anchor),
            }) as Box<dyn Button<Style = ButtonState>>
        }));

        self
    }

    fn internal_add_actions(
        mut self,
        tree: &mut taffy::TaffyTree,
        actions: &[(Arc<str>, Arc<str>)],
        font_system: &mut FontSystem,
    ) -> Self {
        if actions.is_empty() {
            return self;
        }

        self.action_container = tree
            .new_leaf(taffy::Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Row,
                justify_content: Some(taffy::JustifyContent::SpaceEvenly),
                size: taffy::Size {
                    width: auto(),
                    height: auto(),
                },
                ..Default::default()
            })
            .ok();

        let mut buttons = actions
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, action)| {
                let font = &self
                    .context
                    .config
                    .styles
                    .default
                    .buttons
                    .action
                    .default
                    .font;
                let text = text_renderer::Text::new(font, font_system, &action.1);

                Box::new(ActionButton {
                    node: tree.new_leaf(taffy::Style::DEFAULT).unwrap(),
                    context: self.context.clone(),
                    hint: Hint::new(tree, self.context.clone(), "", font_system),
                    text,
                    x: 0.,
                    y: 0.,
                    action: action.0,
                    state: State::Unhovered,
                    width: 0.,
                    tx: self.sender.clone(),
                    index,
                }) as Box<dyn Button<Style = ButtonState>>
            })
            .collect();

        self.buttons.append(&mut buttons);

        self
    }

    #[must_use]
    pub fn buttons(&self) -> &[Box<dyn Button<Style = ButtonState>>] {
        &self.buttons
    }

    pub fn buttons_mut(&mut self) -> &mut [Box<dyn Button<Style = ButtonState>>] {
        &mut self.buttons
    }
}

pub struct Hint {
    node: taffy::NodeId,
    combination: Box<str>,
    text: text_renderer::Text,
    context: components::Context,
    x: f32,
    y: f32,
}

impl Hint {
    pub fn new<T>(
        tree: &mut taffy::TaffyTree<()>,
        context: components::Context,
        combination: T,
        font_system: &mut FontSystem,
    ) -> Self
    where
        T: AsRef<str>,
    {
        let node = tree.new_leaf(taffy::Style::DEFAULT).unwrap();

        Self {
            node,
            combination: combination.as_ref().into(),
            text: text_renderer::Text::new(
                &context.config.styles.default.font,
                font_system,
                combination.as_ref(),
            ),
            context,
            x: 0.,
            y: 0.,
        }
    }
}

impl Component for Hint {
    type Style = config::Hint;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        &self.context.config.styles.hover.hint
    }

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<()>,
        urgency: Urgency,
    ) -> Vec<buffers::Instance> {
        let style = &self.context.config.styles.hover.hint;
        let bounds = self.get_render_bounds(tree);

        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.color(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.color(urgency),
            scale: self.context.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.7,
        }]
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        self.node = tree.new_leaf(taffy::Style::DEFAULT).unwrap();
        // TODO: make it actually calculate
    }

    fn apply_computed_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
    }

    fn get_text_areas(&self, tree: &taffy::TaffyTree<()>, urgency: Urgency) -> Vec<TextArea<'_>> {
        let style = self.get_style();
        let text_extents = self.text.get_bounds();
        let bounds = self.get_render_bounds(tree);

        let remaining_padding = style.width.resolve(text_extents.width) - text_extents.width;
        let (pl, _) = match (style.padding.left.is_auto(), style.padding.right.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.right.resolve(0.)),
            _ => (
                style.padding.left.resolve(0.),
                style.padding.right.resolve(0.),
            ),
        };
        let remaining_padding = style.height.resolve(text_extents.height) - text_extents.height;
        let (pt, _) = match (style.padding.top.is_auto(), style.padding.bottom.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.bottom.resolve(0.)),
            _ => (
                style.padding.top.resolve(0.),
                style.padding.bottom.resolve(0.),
            ),
        };

        vec![TextArea {
            buffer: &self.text.buffer,
            left: bounds.x + style.padding.left.resolve(pl),
            top: bounds.y + style.padding.top.resolve(pt),
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            bounds: glyphon::TextBounds {
                left: (bounds.x + style.padding.left.resolve(pl)) as i32,
                top: (bounds.y + style.padding.top.resolve(pt)) as i32,
                right: (bounds.x + style.padding.left.resolve(pl) + bounds.width) as i32,
                bottom: (bounds.y + style.padding.top.resolve(pt) + bounds.height) as i32,
            },
            default_color: style.font.color.into_glyphon(urgency),
            custom_glyphs: &[],
        }]
    }

    fn get_textures(&self, _: &taffy::TaffyTree<()>) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
    }
}

#[cfg(test)]
mod tests {
    use super::ButtonManager;
    use crate::{Urgency, components, manager::UiState};
    use glyphon::FontSystem;

    #[test]
    fn test_button_click_detection() {
        let mut font_system = FontSystem::new();

        let context = components::Context {
            id: 1,
            app_name: "".into(),
            ui_state: UiState::default(),
            config: crate::config::Config::default().into(),
        };
        let mut button_manager = ButtonManager::new(context, Urgency::Normal, None)
            .add_dismiss(&mut font_system)
            .finish(&mut font_system);

        let button = &mut button_manager.buttons_mut()[0];
        button.update_layout(10.0, 10.0);

        let style = button.get_style();
        let width = style.width
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right;

        let height = style.height
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right;

        // Define test points: (x, y, should_click)
        let test_points = [
            // Internal point (should click)
            (10.0 + width as f64 / 2.0, 10.0 + height as f64 / 2.0, true),
            // Exact corners (should click)
            (10.0, 10.0, true),                                // Top left
            (10.0 + width as f64, 10.0, true),                 // Top right
            (10.0, 10.0 + height as f64, true),                // Bottom left
            (10.0 + width as f64, 10.0 + height as f64, true), // Bottom right
            // Just outside corners (should not click)
            (10.0 - 0.1, 10.0, false), // Top left
            (10.0, 10.0 - 0.1, false),
            (10.0 + width as f64 + 0.1, 10.0, false), // Top right
            (10.0 + width as f64, 10.0 - 0.1, false),
            (10.0 - 0.1, 10.0 + height as f64, false), // Bottom left
            (10.0, 10.0 + height as f64 + 0.1, false),
            (10.0 + width as f64 + 0.1, 10.0 + height as f64, false), // Bottom right
            (10.0 + width as f64, 10.0 + height as f64 + 0.1, false),
        ];

        test_points
            .iter()
            .enumerate()
            .for_each(|(i, (x, y, expected))| {
                assert_eq!(
                    button_manager.click(*x, *y),
                    *expected,
                    "Test point {i} at ({x}, {y}) failed",
                );
            });
    }

    #[test]
    fn test_button_hover_detection() {
        let mut font_system = FontSystem::new();

        let context = components::Context {
            id: 1,
            app_name: "".into(),
            ui_state: UiState::default(),
            config: crate::config::Config::default().into(),
        };
        let mut button_manager = ButtonManager::new(context, Urgency::Normal, None)
            .add_dismiss(&mut font_system)
            .finish(&mut font_system);

        let button = &mut button_manager.buttons_mut()[0];
        button.update_layout(10.0, 10.0);

        let style = button.get_style();
        let width = style.width
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right;

        let height = style.height
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right;

        // Define test points: (x, y, should_hover)
        let test_points = [
            // Internal point (should hover)
            (10.0 + width as f64 / 2.0, 10.0 + height as f64 / 2.0, true),
            // Exact corners (should hover)
            (10.0, 10.0, true),                                // Top left
            (10.0 + width as f64, 10.0, true),                 // Top right
            (10.0, 10.0 + height as f64, true),                // Bottom left
            (10.0 + width as f64, 10.0 + height as f64, true), // Bottom right
            // Just outside corners (should not hover)
            (10.0 - 0.1, 10.0, false), // Top left
            (10.0, 10.0 - 0.1, false),
            (10.0 + width as f64 + 0.1, 10.0, false), // Top right
            (10.0 + width as f64, 10.0 - 0.1, false),
            (10.0 - 0.1, 10.0 + height as f64, false), // Bottom left
            (10.0, 10.0 + height as f64 + 0.1, false),
            (10.0 + width as f64 + 0.1, 10.0 + height as f64, false), // Bottom right
            (10.0 + width as f64, 10.0 + height as f64 + 0.1, false),
        ];

        test_points
            .iter()
            .enumerate()
            .for_each(|(i, (x, y, expected))| {
                assert_eq!(
                    button_manager.hover(*x, *y),
                    *expected,
                    "Test point {i} at ({x}, {y}) failed",
                );
            });
    }
}
