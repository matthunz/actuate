use super::Theme;
use crate::{
    Data, Scope,
    compose::Compose,
    ecs::spawn,
    ecs::{Modifier, Modify},
    use_context,
};
use bevy_color::Color;
use bevy_ui::{BackgroundColor, BorderColor, BorderRadius, BoxShadow, Node, UiRect, Val};

/// Create a material UI radio button.
pub fn radio_button<'a>() -> RadioButton<'a> {
    RadioButton {
        is_enabled: true,
        inner_radius: 10.,
        outer_radius: 20.,
        border_width: 2.,
        elevation: 0.,
        modifier: Modifier::default(),
    }
}

/// Material UI radio button.
#[derive(Clone, Debug, Data)]
#[actuate(path = "crate")]
pub struct RadioButton<'a> {
    is_enabled: bool,
    inner_radius: f32,
    outer_radius: f32,
    border_width: f32,
    elevation: f32,
    modifier: Modifier<'a>,
}

impl RadioButton<'_> {
    /// Set the enabled state of this radio button.
    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// Set the inner radius of this radio button.
    pub fn inner_radius(mut self, inner_radius: f32) -> Self {
        self.inner_radius = inner_radius;
        self
    }

    /// Set the outer radius of this radio button.
    pub fn outer_radius(mut self, outer_radius: f32) -> Self {
        self.outer_radius = outer_radius;
        self
    }

    /// Set the border width of this radio button.
    pub fn border_width(mut self, border_width: f32) -> Self {
        self.border_width = border_width;
        self
    }

    /// Set the elevation of this radio button.
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = elevation;
        self
    }
}

impl Compose for RadioButton<'_> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let theme = use_context::<Theme>(&cx).cloned().unwrap_or_default();

        let size = Val::Px(cx.me().outer_radius * 2.);
        let inner_size = Val::Px(cx.me().inner_radius * 2.);
        let offset = Val::Px((cx.me().outer_radius - cx.me().inner_radius) - 2.);

        cx.me()
            .modifier
            .apply(spawn((
                Node {
                    width: size,
                    height: size,
                    border: UiRect::all(Val::Px(cx.me().border_width)),
                    border_radius: BorderRadius::MAX,
                    ..Default::default()
                },
                BorderColor::all(theme.colors.primary),
                BoxShadow::new(
                    Color::srgba(0., 0., 0., 0.12 * cx.me().elevation),
                    Val::Px(0.),
                    Val::Px(1.),
                    Val::Px(0.),
                    Val::Px(3. * cx.me().elevation),
                ),
            )))
            .content(if cx.me().is_enabled {
                Some(spawn((
                    Node {
                        width: inner_size,
                        height: inner_size,
                        top: offset,
                        left: offset,
                        border_radius: BorderRadius::MAX,
                        ..Default::default()
                    },
                    BackgroundColor(theme.colors.primary),
                )))
            } else {
                None
            })
    }
}

impl<'a> Modify<'a> for RadioButton<'a> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
