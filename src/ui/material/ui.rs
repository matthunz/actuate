use super::Theme;
use crate::{
    Scope, Signal,
    ecs::{Modifier, Modify, spawn},
    prelude::Compose,
    use_provider,
};
use actuate_macros::Data;
use bevy_ui::{BackgroundColor, FlexDirection, Node, Val};

/// Create a material UI composable.
///
/// This will provide a [`Theme`] and set the background for its content.
pub fn material_ui<'a, C: Compose>(content: C) -> MaterialUi<'a, C> {
    MaterialUi {
        content,
        theme: Theme::default(),
        modifier: Modifier::default(),
    }
}

/// Material UI composable.
///
/// For more see [`material_ui`].
#[derive(Data)]
#[actuate(path = "crate")]
pub struct MaterialUi<'a, C> {
    content: C,
    theme: Theme,
    modifier: Modifier<'a>,
}

impl<'a, C> MaterialUi<'a, C> {
    /// Set the theme of this composable.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl<'a, C: Compose> Compose for MaterialUi<'a, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let theme = use_provider(&cx, || cx.me().theme.clone());

        cx.me()
            .modifier
            .apply(spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Percent(100.),
                    height: Val::Percent(100.),
                    ..Default::default()
                },
                BackgroundColor(theme.colors.background),
            )))
            .content(unsafe { Signal::map_unchecked(cx.me(), |me| &me.content) })
    }
}

impl<'a, C> Modify<'a> for MaterialUi<'a, C> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
