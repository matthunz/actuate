use super::{Theme, container};
use crate::{
    Data, Scope, Signal,
    compose::Compose,
    ecs::{Modifier, Modify},
    use_context,
};
use bevy_color::Color;
use bevy_ui::{BorderRadius, Node, UiRect, Val};

/// Create a material UI button.
pub fn button<'a, C>(content: C) -> Button<'a, C> {
    Button {
        content,
        background_color: None,
        elevation: 0.,
        height: Val::Px(40.),
        padding: UiRect::left(Val::Px(24.)).with_right(Val::Px(24.)),
        modifier: Modifier::default(),
    }
}

/// Material UI button.
#[derive(Clone, Debug, Data)]
#[actuate(path = "crate")]
pub struct Button<'a, C> {
    content: C,
    background_color: Option<Color>,
    padding: UiRect,
    height: Val,
    elevation: f32,
    modifier: Modifier<'a>,
}

impl<'a, C> Button<'a, C> {
    /// Set the background color of this button.
    pub fn background_color(mut self, background_color: Color) -> Self {
        self.background_color = Some(background_color);
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

impl<C: Compose> Compose for Button<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let theme = use_context::<Theme>(&cx).cloned().unwrap_or_default();

        container(unsafe { Signal::map_unchecked(cx.me(), |me| &me.content) })
            .background_color(cx.me().background_color.unwrap_or(theme.colors.primary))
            .border_radius(
                BorderRadius::all(Val::Px(10.))
                    .with_left(Val::Px(20.))
                    .with_right(Val::Px(20.)),
            )
            .on_insert(move |mut entity| {
                let mut node = entity.get_mut::<Node>().unwrap();
                node.height = cx.me().height;
            })
            .append(Signal::map(cx.me(), |me| &me.modifier).into())
    }
}

impl<'a, C: Compose> Modify<'a> for Button<'a, C> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
