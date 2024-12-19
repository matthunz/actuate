use super::Theme;
use crate::{
    Data, Scope,
    compose::Compose,
    ecs::spawn,
    ecs::{Modifier, Modify},
    use_context,
};
use bevy_color::Color;
use bevy_ui::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, BoxShadow, JustifyContent, Node,
    UiRect, Val,
};

/// Create a material UI switch.
pub fn switch<'a>() -> Switch<'a> {
    Switch {
        is_checked: false,
        inner_radius: 10.,
        outer_radius: 20.,
        border_width: 2.,
        elevation: 0.,
        modifier: Modifier::default(),
    }
}

/// Material UI switch.
#[derive(Clone, Debug, Data)]
#[actuate(path = "crate")]
pub struct Switch<'a> {
    is_checked: bool,
    inner_radius: f32,
    outer_radius: f32,
    border_width: f32,
    elevation: f32,
    modifier: Modifier<'a>,
}

impl Switch<'_> {
    /// Set the checked state of this switch.
    ///
    /// When `true`, the knob is aligned to the end of the track.
    pub fn is_checked(mut self, is_checked: bool) -> Self {
        self.is_checked = is_checked;
        self
    }

    /// Set the knob radius of this switch.
    pub fn inner_radius(mut self, inner_radius: f32) -> Self {
        self.inner_radius = inner_radius;
        self
    }

    /// Set the track radius of this switch.
    pub fn outer_radius(mut self, outer_radius: f32) -> Self {
        self.outer_radius = outer_radius;
        self
    }

    /// Set the border width of this switch.
    pub fn border_width(mut self, border_width: f32) -> Self {
        self.border_width = border_width;
        self
    }

    /// Set the elevation of this switch.
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
        let padding = Val::Px((cx.me().outer_radius - cx.me().inner_radius) - 2.);

        cx.me()
            .modifier
            .apply(spawn((
                Node {
                    width,
                    height,
                    border: UiRect::all(Val::Px(cx.me().border_width)),
                    padding: UiRect::all(padding),
                    align_items: AlignItems::Center,
                    justify_content: if cx.me().is_checked {
                        JustifyContent::FlexEnd
                    } else {
                        JustifyContent::FlexStart
                    },
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
            .content(spawn((
                Node {
                    width: knob_size,
                    height: knob_size,
                    border_radius: BorderRadius::MAX,
                    ..Default::default()
                },
                BackgroundColor(theme.colors.primary),
            )))
    }
}

impl<'a> Modify<'a> for Switch<'a> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
