use super::Theme;
use crate::{
    Data, Scope, Signal,
    compose::Compose,
    ecs::spawn,
    ecs::{Modifier, Modify},
    use_context,
};
use bevy_color::Color;
use bevy_ui::{
    AlignItems, BackgroundColor, BorderRadius, BoxShadow, FlexDirection, JustifyContent, Node,
    Overflow, UiRect, Val,
};

/// Create a material UI container.
pub fn container<'a, C>(content: C) -> Container<'a, C> {
    Container {
        content,
        elevation: 0.,
        padding: UiRect::all(Val::Px(12.))
            .with_left(Val::Px(24.))
            .with_right(Val::Px(24.)),
        background_color: None,
        border_radius: BorderRadius::all(Val::Px(12.)),
        modifier: Modifier::default(),
    }
}

/// Material UI container.
///
/// For more see [`container`].
#[derive(Clone, Debug, Data)]
#[actuate(path = "crate")]
pub struct Container<'a, C> {
    content: C,
    padding: UiRect,
    elevation: f32,
    modifier: Modifier<'a>,
    background_color: Option<Color>,
    border_radius: BorderRadius,
}

impl<'a, C> Container<'a, C> {
    /// Set the background color of this button.
    pub fn background_color(mut self, background_color: Color) -> Self {
        self.background_color = Some(background_color);
        self
    }

    /// Set the border radius of this button.
    pub fn border_radius(mut self, border_radius: BorderRadius) -> Self {
        self.border_radius = border_radius;
        self
    }

    /// Set the elevation of this button.
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = elevation;
        self
    }

    /// Set the padding of this button.
    pub fn padding(mut self, padding: UiRect) -> Self {
        self.padding = padding;
        self
    }
}

impl<C: Compose> Compose for Container<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let theme = use_context::<Theme>(&cx).cloned().unwrap_or_default();

        cx.me()
            .modifier
            .apply(spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: cx.me().padding,
                    overflow: Overflow::clip(),
                    border_radius: cx.me().border_radius,
                    ..Default::default()
                },
                BackgroundColor(
                    cx.me()
                        .background_color
                        .unwrap_or(theme.colors.surface_container),
                ),
                BoxShadow::new(
                    Color::srgba(0., 0., 0., 0.12 * cx.me().elevation),
                    Val::Px(0.),
                    Val::Px(1.),
                    Val::Px(0.),
                    Val::Px(3. * cx.me().elevation),
                ),
            )))
            .content(unsafe { Signal::map_unchecked(cx.me(), |me| &me.content) })
    }
}

impl<'a, C: Compose> Modify<'a> for Container<'a, C> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
