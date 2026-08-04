use crate::{
    Scope, Signal, SignalMut,
    ecs::{Modifier, Modify, spawn, use_world},
    prelude::Compose,
    use_mut,
};
use actuate_macros::Data;
use bevy_ecs::prelude::*;
use bevy_input::{
    mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
};
use bevy_picking::prelude::*;
use bevy_ui::prelude::*;
use std::mem;

#[cfg(feature = "material")]
#[cfg_attr(docsrs, doc(cfg(feature = "material")))]
/// Material UI.
pub mod material;

mod text_input;
pub use self::text_input::{TextInput, text_input};

/// Create a scroll view.
pub fn scroll_view<'a, C: Compose>(content: C) -> ScrollView<'a, C> {
    ScrollView {
        content,
        line_size: 30.,
        modifier: Modifier::default(),
        scroll_x: true,
        scroll_y: true,
    }
}

#[derive(Data)]
#[actuate(path = "crate")]
/// Scroll view composable.
pub struct ScrollView<'a, C> {
    content: C,
    line_size: f32,
    scroll_x: bool,
    scroll_y: bool,
    modifier: Modifier<'a>,
}

impl<C> ScrollView<'_, C> {
    /// Set the line size to scroll (default: 30).
    pub fn line_size(mut self, size: f32) -> Self {
        self.line_size = size;
        self
    }

    /// Enable or disable horizontal scrolling (default: true).
    pub fn scroll_x(mut self, scroll_x: bool) -> Self {
        self.scroll_x = scroll_x;
        self
    }

    /// Enable or disable vertical scrolling (default: true).
    pub fn scroll_y(mut self, scroll_y: bool) -> Self {
        self.scroll_y = scroll_y;
        self
    }
}

impl<C: Compose> Compose for ScrollView<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let is_hovered = use_mut(&cx, || false);

        let entity_cell = use_mut(&cx, || None);

        use_world(
            &cx,
            move |mut mouse_wheel_events: MessageReader<MouseWheel>,
                  mut scrolled_node_query: Query<&mut ScrollPosition>,
                  keyboard_input: Res<ButtonInput<KeyCode>>| {
                for mouse_wheel_event in mouse_wheel_events.read() {
                    let (mut dx, mut dy) = match mouse_wheel_event.unit {
                        MouseScrollUnit::Line => (
                            mouse_wheel_event.x * cx.me().line_size,
                            mouse_wheel_event.y * cx.me().line_size,
                        ),
                        MouseScrollUnit::Pixel => (mouse_wheel_event.x, mouse_wheel_event.y),
                    };

                    if cx.me().scroll_x
                        && cx.me().scroll_y
                        && (keyboard_input.pressed(KeyCode::ControlLeft)
                            || keyboard_input.pressed(KeyCode::ControlRight))
                    {
                        std::mem::swap(&mut dx, &mut dy)
                    }

                    if *is_hovered
                        && let Some(entity) = *entity_cell
                        && let Ok(mut scroll_position) = scrolled_node_query.get_mut(entity)
                    {
                        if cx.me().scroll_x {
                            scroll_position.x -= dx;
                        }

                        if cx.me().scroll_y {
                            scroll_position.y -= dy;
                        }
                    }
                }
            },
        );

        let modifier = &cx.me().modifier;
        let modifier: &Modifier = unsafe { mem::transmute(modifier) };

        modifier
            .apply(
                spawn(Node {
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                    ..Default::default()
                })
                .on_spawn(move |entity| SignalMut::set(entity_cell, Some(entity.id())))
                .observe(move |_: On<Pointer<Over>>| SignalMut::set(is_hovered, true))
                .observe(move |_: On<Pointer<Out>>| SignalMut::set(is_hovered, false)),
            )
            .content(unsafe { Signal::map_unchecked(cx.me(), |me| &me.content) })
    }
}

impl<'a, C: Compose> Modify<'a> for ScrollView<'a, C> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
