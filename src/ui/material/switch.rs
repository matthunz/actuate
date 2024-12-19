use crate::ui::material::Theme;
use crate::{
    compose::Compose,
    ecs::spawn,
    ecs::{Modifier, Modify},
    use_context, Data, Scope,
};
use bevy_color::Color;
use bevy_ui::{BackgroundColor, BorderColor, BorderRadius, BoxShadow, Node, UiRect, Val};

/// Create a material UI switch.
pub fn switch<'a>() -> Switch<'a> {
    Switch {
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
pub struct Switch<'a> {
    is_enabled: bool,
    inner_radius: f32,
    outer_radius: f32,
    border_width: f32,
    elevation: f32,
    modifier: Modifier<'a>,
}

impl Switch<'_> {
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

impl Compose for Switch<'_> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let theme = use_context::<Theme>(&cx).cloned().unwrap_or_default();

        let height = Val::Px(cx.me().outer_radius * 2.);
        let width = Val::Px(cx.me().outer_radius * 3.);
        let knob_size = Val::Px(cx.me().inner_radius * 2.);
        let padding = (cx.me().outer_radius - cx.me().inner_radius) - 2.;
        let padding_right = padding + 2. * cx.me().inner_radius;
        let mut padding_rect = UiRect::all(Val::Px(padding));
        padding_rect.right = Val::Px(padding_right);
        let offset_left_val = if cx.me().is_enabled {
            Val::Percent(0.)
        } else {
            Val::Percent(100.)
        };

        cx.me()
            .modifier
            .apply(spawn((
                Node {
                    width,
                    height,
                    border: UiRect::all(Val::Px(cx.me().border_width)),
                    padding: padding_rect,
                    ..Default::default()
                },
                BorderRadius::MAX,
                BorderColor(theme.colors.primary),
                BoxShadow {
                    color: Color::srgba(0., 0., 0., 0.12 * cx.me().elevation),
                    x_offset: Val::Px(0.),
                    y_offset: Val::Px(1.),
                    spread_radius: Val::Px(0.),
                    blur_radius: Val::Px(3. * cx.me().elevation),
                },
            )))
            .content(Some(spawn((
                Node {
                    width: knob_size,
                    height: knob_size,
                    left: offset_left_val,

                    ..Default::default()
                },
                BackgroundColor(theme.colors.primary),
                BorderRadius::MAX,
            ))))
    }
}

impl<'a> Modify<'a> for Switch<'a> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
