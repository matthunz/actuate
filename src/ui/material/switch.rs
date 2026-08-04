use super::Theme;
use crate::{
    Data, Scope, SignalMut,
    compose::Compose,
    ecs::spawn,
    ecs::{Modifier, Modify},
    use_context, use_mut,
};
use bevy_color::{Alpha, Color};
use bevy_ecs::prelude::*;
use bevy_picking::prelude::*;
use bevy_ui::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, BoxShadow, JustifyContent, Node,
    PositionType, UiRect, Val,
};
use std::mem;

/// Create a material UI switch.
pub fn switch<'a>() -> Switch<'a> {
    Switch {
        is_checked: false,
        is_enabled: true,
        track_width: 52.,
        track_height: 32.,
        border_width: 2.,
        handle_size: 16.,
        selected_handle_size: 24.,
        pressed_handle_size: 28.,
        state_layer_size: 40.,
        elevation: 0.,
        modifier: Modifier::default(),
    }
}

/// Material UI switch.
///
/// Dimensions default to the Material 3 switch tokens: a 52x32 track with a
/// 16px handle that grows to 24px when checked and 28px while pressed.
#[derive(Clone, Debug, Data)]
#[actuate(path = "crate")]
pub struct Switch<'a> {
    is_checked: bool,
    is_enabled: bool,
    track_width: f32,
    track_height: f32,
    border_width: f32,
    handle_size: f32,
    selected_handle_size: f32,
    pressed_handle_size: f32,
    state_layer_size: f32,
    elevation: f32,
    modifier: Modifier<'a>,
}

impl Switch<'_> {
    /// Set the checked state of this switch.
    ///
    /// When `true`, the track is filled and the handle is aligned to its end.
    pub fn is_checked(mut self, is_checked: bool) -> Self {
        self.is_checked = is_checked;
        self
    }

    /// Set the enabled state of this switch.
    ///
    /// A disabled switch is dimmed and does not respond to hover or press.
    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// Set the width of this switch's track.
    pub fn track_width(mut self, track_width: f32) -> Self {
        self.track_width = track_width;
        self
    }

    /// Set the height of this switch's track.
    pub fn track_height(mut self, track_height: f32) -> Self {
        self.track_height = track_height;
        self
    }

    /// Set the width of this switch's track outline.
    ///
    /// The outline is only drawn while the switch is unchecked.
    pub fn border_width(mut self, border_width: f32) -> Self {
        self.border_width = border_width;
        self
    }

    /// Set the diameter of this switch's handle while unchecked.
    pub fn handle_size(mut self, handle_size: f32) -> Self {
        self.handle_size = handle_size;
        self
    }

    /// Set the diameter of this switch's handle while checked.
    pub fn selected_handle_size(mut self, selected_handle_size: f32) -> Self {
        self.selected_handle_size = selected_handle_size;
        self
    }

    /// Set the diameter of this switch's handle while pressed.
    pub fn pressed_handle_size(mut self, pressed_handle_size: f32) -> Self {
        self.pressed_handle_size = pressed_handle_size;
        self
    }

    /// Set the diameter of this switch's hover and press state layer.
    pub fn state_layer_size(mut self, state_layer_size: f32) -> Self {
        self.state_layer_size = state_layer_size;
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

        let is_hovered = use_mut(&cx, || false);
        let is_pressed = use_mut(&cx, || false);

        let is_enabled = cx.me().is_enabled;
        let is_checked = cx.me().is_checked;

        let handle_size = if is_enabled && *is_pressed {
            cx.me().pressed_handle_size
        } else if is_checked {
            cx.me().selected_handle_size
        } else {
            cx.me().handle_size
        };

        // A checked track is filled rather than outlined.
        let border_width = if is_checked { 0. } else { cx.me().border_width };

        let (track_color, border_color, handle_color) = match (is_enabled, is_checked) {
            (true, true) => (theme.colors.primary, Color::NONE, theme.colors.on_primary),
            (true, false) => (
                theme.colors.surface_container_highest,
                theme.colors.outline,
                theme.colors.outline,
            ),
            (false, true) => (
                theme.colors.on_surface.with_alpha(0.12),
                Color::NONE,
                theme.colors.surface,
            ),
            (false, false) => (
                theme.colors.surface_container_highest.with_alpha(0.12),
                theme.colors.on_surface.with_alpha(0.12),
                theme.colors.on_surface.with_alpha(0.38),
            ),
        };

        // The handle is inset equally on every side, so the track's padding
        // shrinks as the handle grows.
        let padding = ((cx.me().track_height - handle_size) / 2. - border_width).max(0.);

        let state_layer_color = if !is_enabled {
            Color::NONE
        } else {
            let base = if is_checked {
                theme.colors.primary
            } else {
                theme.colors.on_surface
            };

            if *is_pressed {
                base.with_alpha(0.1)
            } else if *is_hovered {
                base.with_alpha(0.08)
            } else {
                Color::NONE
            }
        };

        let state_layer_size = cx.me().state_layer_size;
        let state_layer_offset = Val::Px((handle_size - state_layer_size) / 2.);

        // Erase the link between the scope's lifetime and the observers below,
        // which would otherwise make `Scope<'_, Switch<'_>>` invariant.
        let modifier = &cx.me().modifier;
        let modifier: &Modifier = unsafe { mem::transmute(modifier) };

        modifier
            .apply(
                spawn((
                    Node {
                        width: Val::Px(cx.me().track_width),
                        height: Val::Px(cx.me().track_height),
                        border: UiRect::all(Val::Px(border_width)),
                        padding: UiRect::all(Val::Px(padding)),
                        align_items: AlignItems::Center,
                        justify_content: if is_checked {
                            JustifyContent::FlexEnd
                        } else {
                            JustifyContent::FlexStart
                        },
                        border_radius: BorderRadius::MAX,
                        ..Default::default()
                    },
                    BackgroundColor(track_color),
                    BorderColor::all(border_color),
                    BoxShadow::new(
                        Color::srgba(0., 0., 0., 0.12 * cx.me().elevation),
                        Val::Px(0.),
                        Val::Px(1.),
                        Val::Px(0.),
                        Val::Px(3. * cx.me().elevation),
                    ),
                ))
                .observe(move |_: On<Pointer<Over>>| SignalMut::set(is_hovered, true))
                .observe(move |_: On<Pointer<Out>>| {
                    SignalMut::set(is_hovered, false);
                    SignalMut::set(is_pressed, false);
                })
                .observe(move |_: On<Pointer<Press>>| SignalMut::set(is_pressed, true))
                .observe(move |_: On<Pointer<Release>>| SignalMut::set(is_pressed, false)),
            )
            .content(
                spawn((
                    Node {
                        width: Val::Px(handle_size),
                        height: Val::Px(handle_size),
                        border_radius: BorderRadius::MAX,
                        ..Default::default()
                    },
                    BackgroundColor(handle_color),
                ))
                .content(spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(state_layer_size),
                        height: Val::Px(state_layer_size),
                        left: state_layer_offset,
                        top: state_layer_offset,
                        border_radius: BorderRadius::MAX,
                        ..Default::default()
                    },
                    BackgroundColor(state_layer_color),
                ))),
            )
    }
}

impl<'a> Modify<'a> for Switch<'a> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
