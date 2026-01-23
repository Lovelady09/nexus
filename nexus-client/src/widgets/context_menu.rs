//! Lazy context menu widget
//!
//! This is a patched version of iced_aw's ContextMenu that doesn't eagerly
//! build the overlay content in `children()` and `diff()`. The original
//! implementation calls the overlay closure on every frame for tree diffing,
//! which causes severe performance issues when many context menus exist
//! (e.g., one per row in a file table with hundreds of files).
//!
//! The fix: only build the overlay tree when the menu is actually shown.

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{
    Clipboard, Layout, Shell, Widget,
    layout::{Limits, Node},
    mouse::{self, Button, Cursor},
    overlay, renderer,
};
use iced::{Background, Color, Element, Event, Length, Point, Rectangle, Size, Vector};

/// The style of a [`LazyContextMenu`].
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// The background of the overlay (typically transparent).
    pub background: Background,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: Background::Color(Color::TRANSPARENT),
        }
    }
}

/// Style catalog for [`LazyContextMenu`].
pub trait Catalog {
    /// Style class type.
    type Class<'a>;

    /// The default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Get the style for a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// Style function type.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_theme| Style {
            background: Background::Color(Color::TRANSPARENT),
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// A context menu that lazily builds its overlay content.
///
/// Unlike iced_aw's ContextMenu, this widget only builds the overlay
/// content when the menu is actually displayed (on right-click),
/// not on every frame during tree diffing.
#[allow(missing_debug_implementations)]
pub struct LazyContextMenu<'a, Overlay, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Overlay: Fn() -> Element<'a, Message, Theme, Renderer>,
    Message: Clone,
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    /// The underlying element.
    underlay: Element<'a, Message, Theme, Renderer>,
    /// The content builder for the overlay (called lazily).
    overlay: Overlay,
    /// The style class.
    class: Theme::Class<'a>,
}

impl<'a, Overlay, Message, Theme, Renderer> LazyContextMenu<'a, Overlay, Message, Theme, Renderer>
where
    Overlay: Fn() -> Element<'a, Message, Theme, Renderer>,
    Message: Clone,
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    /// Creates a new [`LazyContextMenu`].
    ///
    /// `underlay`: The element that triggers the context menu on right-click.
    /// `overlay`: A closure that builds the menu content (called only when shown).
    pub fn new<U>(underlay: U, overlay: Overlay) -> Self
    where
        U: Into<Element<'a, Message, Theme, Renderer>>,
    {
        LazyContextMenu {
            underlay: underlay.into(),
            overlay,
            class: Theme::default(),
        }
    }

    /// Sets the style of the [`LazyContextMenu`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }
}

/// Internal state for the context menu.
#[derive(Debug, Default)]
struct State {
    /// Whether the menu is currently shown.
    show: bool,
    /// The cursor position when right-clicked (menu appears here).
    cursor_position: Point,
    /// Tree for the overlay content (lazily initialized when menu opens).
    overlay_tree: Option<Tree>,
}

impl State {
    const fn new() -> Self {
        Self {
            show: false,
            cursor_position: Point::ORIGIN,
            overlay_tree: None,
        }
    }
}

impl<'a, Content, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for LazyContextMenu<'a, Content, Message, Theme, Renderer>
where
    Content: 'a + Fn() -> Element<'a, Message, Theme, Renderer>,
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: Catalog,
{
    fn size(&self) -> Size<Length> {
        self.underlay.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.underlay
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.underlay.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

    fn children(&self) -> Vec<Tree> {
        // KEY FIX: Only create tree for underlay, NOT for overlay.
        // The overlay tree is created lazily in overlay() when needed.
        // This avoids calling (self.overlay)() on every frame.
        vec![Tree::new(&self.underlay)]
    }

    fn diff(&self, tree: &mut Tree) {
        // KEY FIX: Only diff the underlay, not the overlay.
        // Original iced_aw calls (self.overlay)() here which is expensive.
        if tree.children.len() == 1 {
            tree.diff_children(&[&self.underlay]);
        } else {
            // Tree was created with old widget that had overlay child,
            // rebuild with just underlay
            tree.children = vec![Tree::new(&self.underlay)];
        }
    }

    fn operate(
        &mut self,
        state: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.underlay
            .as_widget_mut()
            .operate(&mut state.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Handle right-click to show menu
        if *event == Event::Mouse(mouse::Event::ButtonPressed(Button::Right)) {
            let bounds = layout.bounds();

            if cursor.is_over(bounds) {
                let s: &mut State = state.state.downcast_mut();
                s.cursor_position = cursor.position().unwrap_or_default();
                s.show = true;
                // Reset overlay tree so it gets rebuilt with fresh content
                s.overlay_tree = None;
                shell.capture_event();
            }
        }

        // Forward events to underlay
        self.underlay.as_widget_mut().update(
            &mut state.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.underlay.as_widget().mouse_interaction(
            &state.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let s: &mut State = tree.state.downcast_mut();

        if !s.show {
            // Menu not shown - delegate to underlay's overlay if any
            return self.underlay.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            );
        }

        // Menu is shown - NOW we build the overlay content
        let position = s.cursor_position + translation;
        let content = (self.overlay)();

        // Initialize or update the overlay tree
        if s.overlay_tree.is_none() {
            s.overlay_tree = Some(Tree::new(&content));
        }
        // Diff the content to update widget state
        if let Some(ref mut overlay_tree) = s.overlay_tree {
            content.as_widget().diff(overlay_tree);
        }

        Some(overlay::Element::new(Box::new(ContextMenuOverlay {
            position,
            content,
            state: s,
        })))
    }
}

impl<'a, Content, Message, Theme, Renderer>
    From<LazyContextMenu<'a, Content, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Content: 'a + Fn() -> Element<'a, Message, Theme, Renderer>,
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: 'a + Catalog,
{
    fn from(menu: LazyContextMenu<'a, Content, Message, Theme, Renderer>) -> Self {
        Element::new(menu)
    }
}

/// The overlay element for the context menu.
struct ContextMenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Message: Clone,
{
    position: Point,
    content: Element<'a, Message, Theme, Renderer>,
    state: &'b mut State,
}

impl<'a, 'b, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for ContextMenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: 'a,
{
    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        let Some(content_layout) = layout.children().next() else {
            return;
        };

        let Some(tree) = self.state.overlay_tree.as_mut() else {
            return;
        };

        self.content
            .as_widget_mut()
            .operate(tree, content_layout, renderer, operation);
    }

    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let limits = Limits::new(Size::ZERO, bounds);

        let Some(tree) = self.state.overlay_tree.as_mut() else {
            // Return empty layout if tree not initialized
            return Node::new(bounds);
        };
        let mut content = self.content.as_widget_mut().layout(tree, renderer, &limits);

        // Adjust position to stay within bounds
        let mut position = self.position;
        if position.x + content.size().width > bounds.width {
            position.x = f32::max(0.0, position.x - content.size().width);
        }
        if position.y + content.size().height > bounds.height {
            position.y = f32::max(0.0, position.y - content.size().height);
        }

        content.move_to_mut(position);

        Node::with_children(bounds, vec![content])
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        let Some(content_layout) = layout.children().next() else {
            return;
        };

        let Some(tree) = self.state.overlay_tree.as_ref() else {
            return;
        };

        self.content.as_widget().draw(
            tree,
            renderer,
            theme,
            style,
            content_layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let Some(content_layout) = layout.children().next() else {
            return;
        };

        // Handle escape to close
        if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event
            && *key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
        {
            self.state.show = false;
            shell.capture_event();
            return;
        }

        // Check if clicked outside menu
        let clicked_outside = matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left | mouse::Button::Right
            )) | Event::Touch(iced::touch::Event::FingerPressed { .. })
        ) && !cursor.is_over(content_layout.bounds());

        if clicked_outside {
            self.state.show = false;
            return;
        }

        // Note: Hover states don't work because Button stores its status in self.status
        // (on the widget struct) rather than in the tree state. Since we recreate the
        // content Element each frame, the button's status is always None/fresh.
        // This is the same limitation as the original iced_aw ContextMenu.

        // Forward event to content so buttons receive it (including hover events)
        let Some(tree) = self.state.overlay_tree.as_mut() else {
            return;
        };

        self.content.as_widget_mut().update(
            tree,
            event,
            content_layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );

        // Close menu after left button release (action was triggered)
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
        ) && cursor.is_over(content_layout.bounds())
        {
            self.state.show = false;
        }

        // Close on window resize
        if matches!(event, Event::Window(iced::window::Event::Resized { .. })) {
            self.state.show = false;
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let Some(content_layout) = layout.children().next() else {
            return mouse::Interaction::None;
        };

        let Some(tree) = self.state.overlay_tree.as_ref() else {
            return mouse::Interaction::None;
        };

        self.content.as_widget().mouse_interaction(
            tree,
            content_layout,
            cursor,
            &layout.bounds(),
            renderer,
        )
    }
}
