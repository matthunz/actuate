use crate::{
    Scope, ScopeState, SignalMut,
    ecs::{Modifier, Modify, spawn, use_world},
    prelude::Compose,
    use_mut, use_ref,
};
use actuate_macros::Data;
use bevy_color::{Alpha, Color};
use bevy_ecs::prelude::*;
use bevy_input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex};
use bevy_picking::prelude::*;
use bevy_text::{EditableText, LineBreak, TextColor, TextCursorStyle, TextLayout};
use bevy_ui::prelude::*;
use std::{cell::RefCell, mem, rc::Rc};

/// The text color to fall back on when there's no [`Theme`](crate::ecs::ui::material::Theme) to read.
///
/// This matches the theme default, so an unthemed input still reads against a light background.
const DEFAULT_COLOR: Color = Color::BLACK;

/// The theme's text color, or [`DEFAULT_COLOR`] when there's no theme.
fn theme_color(cx: ScopeState<'_>) -> Color {
    #[cfg(feature = "material")]
    let color = crate::use_context::<crate::ecs::ui::material::Theme>(cx)
        .map(|theme| theme.colors.text)
        .unwrap_or(DEFAULT_COLOR);

    #[cfg(not(feature = "material"))]
    let color = {
        let _ = cx;
        DEFAULT_COLOR
    };

    color
}

/// Create a text input.
///
/// This is an unstyled primitive wrapping Bevy's [`EditableText`], which brings selection,
/// word-wise motion, clipboard, IME, and glyph-accurate caret placement with it.
///
/// The value is *controlled*: the input renders [`TextInput::value`] and reports edits through
/// [`TextInput::on_change`]. A caller that doesn't write the reported value back will see the
/// input revert, which is what makes rejecting or transforming input possible.
///
/// # Plugins
///
/// Editing is driven by Bevy's `EditableTextInputPlugin` and `InputDispatchPlugin`, which
/// handle keyboard input, focus, IME, and caret placement. Both are part of `DefaultPlugins`.
/// An app built from `MinimalPlugins` has to add `UiWidgetsPlugins` itself, or the input will
/// render but not accept input.
///
/// # Examples
///
/// ```no_run
/// use actuate::ecs::prelude::*;
///
/// #[derive(Data)]
/// struct Form;
///
/// impl Compose for Form {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         let name = use_mut(&cx, String::new);
///
///         text_input()
///             .value(&*name)
///             .placeholder("Name")
///             .on_change(move |value| SignalMut::set(name, value))
///     }
/// }
/// ```
pub fn text_input<'a>() -> TextInput<'a> {
    TextInput {
        value: String::new(),
        placeholder: None,
        on_change: Rc::new(|_| {}),
        is_enabled: true,
        max_characters: None,
        color: None,
        placeholder_color: None,
        modifier: Modifier::default(),
    }
}

/// Text input composable.
///
/// For more see [`text_input`].
#[derive(Clone, Data)]
#[actuate(path = "crate")]
pub struct TextInput<'a> {
    value: String,
    placeholder: Option<String>,
    on_change: Rc<dyn Fn(String) + 'a>,
    is_enabled: bool,
    max_characters: Option<usize>,
    color: Option<Color>,
    placeholder_color: Option<Color>,
    modifier: Modifier<'a>,
}

impl<'a> TextInput<'a> {
    /// Set the text displayed by this input.
    ///
    /// Changing this replaces the editor's contents, so it should be kept in sync with
    /// [`TextInput::on_change`].
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    /// Set the text shown while this input is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set the handler called with the new value whenever this input is edited.
    ///
    /// This is called once per distinct value, and never while an IME composition is in
    /// progress.
    pub fn on_change(mut self, on_change: impl Fn(String) + 'a) -> Self {
        self.on_change = Rc::new(on_change);
        self
    }

    /// Set the enabled state of this input.
    ///
    /// A disabled input can't be focused by pointer or by tab navigation, and loses focus if it
    /// held it.
    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// Set the maximum number of characters this input will accept.
    ///
    /// Edits that would exceed the maximum are ignored. This doesn't truncate a longer
    /// [`TextInput::value`].
    pub fn max_characters(mut self, max_characters: usize) -> Self {
        self.max_characters = Some(max_characters);
        self
    }

    /// Set the color of this input's text and caret.
    ///
    /// Defaults to the [`Theme`](crate::ecs::ui::material::Theme)'s text color, or black when
    /// there's no theme in context.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set the color of this input's placeholder.
    ///
    /// Defaults to a dimmed [`TextInput::color`].
    pub fn placeholder_color(mut self, placeholder_color: Color) -> Self {
        self.placeholder_color = Some(placeholder_color);
        self
    }
}

impl Compose for TextInput<'_> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let theme_color = theme_color(&cx);

        let entity_cell = use_mut(&cx, || None);

        // The last value handed to `on_change`, used to report each edit exactly once. This is
        // a `RefCell` rather than a signal because `on_insert` has to update it too, and a
        // signal write wouldn't land until after the next frame's listeners had already run.
        let last_reported = use_ref(&cx, || RefCell::new(String::new()));

        // Bumped on every edit purely to schedule the re-compose that runs `on_insert`. Without
        // it, a caller that ignores `on_change` would never see the editor reconciled.
        let revision = use_mut(&cx, || 0u64);

        use_world(&cx, move |query: Query<&EditableText>| {
            let me: &Self = &cx.me();

            if let Some(entity) = *entity_cell
                && let Ok(editable) = query.get(entity)
                // Reporting a half-typed composition would send the caller garbage.
                && !editable.is_composing()
            {
                let current = editable.value().to_string();
                if current != *last_reported.borrow() {
                    *last_reported.borrow_mut() = current.clone();

                    SignalMut::update(revision, |revision| *revision += 1);

                    (me.on_change)(current);
                }
            }
        });

        // `Signal` has a private `value` field of its own, which would shadow this composable's
        // when reached through `cx.me()`.
        let me: &Self = &cx.me();

        let is_enabled = me.is_enabled;
        let max_characters = me.max_characters;
        let value = me.value.clone();
        let initial_value = value.clone();

        let color = me.color.unwrap_or(theme_color);
        let placeholder_color = me.placeholder_color.unwrap_or(color.with_alpha(0.6));

        // The placeholder stands in for the editor's own text, so it only shows while there is
        // none.
        let placeholder = if value.is_empty() {
            me.placeholder.clone()
        } else {
            None
        };

        // Erase the link between the scope's lifetime and the observer below, which would
        // otherwise make `Scope<'_, TextInput<'_>>` invariant.
        let modifier = &cx.me().modifier;
        let modifier: &Modifier = unsafe { mem::transmute(modifier) };

        modifier
            .apply(
                spawn(Node {
                    align_items: AlignItems::Center,
                    ..Default::default()
                })
                // Focus the editor when a press lands on this node's padding rather than on the
                // text. Presses on the text itself are handled by Bevy, which also places the
                // caret at the click point.
                // `InputFocus` is `Option`al so this doesn't bring down an app that never added
                // the focus plugin.
                .observe(
                    move |_: On<Pointer<Press>>, focus: Option<ResMut<InputFocus>>| {
                        if is_enabled
                            && let Some(mut focus) = focus
                            && let Some(entity) = *entity_cell
                        {
                            focus.set(entity, FocusCause::Pressed);
                        }
                    },
                ),
            )
            .content((
                spawn((
                    Node {
                        flex_grow: 1.,
                        ..Default::default()
                    },
                    TextLayout {
                        linebreak: LineBreak::NoWrap,
                        ..Default::default()
                    },
                    TextColor(color),
                    // `bevy_ui_render` queries this without an `Option`, so leaving it off means
                    // no caret or selection highlight is ever drawn.
                    TextCursorStyle {
                        color,
                        ..Default::default()
                    },
                    if is_enabled {
                        Pickable::default()
                    } else {
                        Pickable::IGNORE
                    },
                ))
                // `EditableText` holds the live buffer, caret, and selection, so it can't go in
                // the bundle above: `spawn` re-inserts that on every re-compose, which would
                // reset the editor while the user types.
                .on_spawn(move |mut entity| {
                    let mut editable = EditableText::new(&initial_value);
                    editable.max_characters = max_characters;

                    entity.insert(editable);

                    SignalMut::set(entity_cell, Some(entity.id()));
                })
                .on_insert(move |mut entity| {
                    let id = entity.id();

                    if let Some(mut editable) = entity.get_mut::<EditableText>() {
                        if editable.max_characters != max_characters {
                            editable.max_characters = max_characters;
                        }

                        // Push the value down only when it actually diverged, so an edit that
                        // has already round-tripped through the caller doesn't move the caret,
                        // and never mid-composition, which would destroy the IME preedit.
                        if !editable.is_composing() && editable.value().to_string() != value {
                            editable.editor.set_text(&value);

                            // This write is ours, not the user's, so it must not come back out
                            // of `on_change` as a fresh edit.
                            *last_reported.borrow_mut() = value.clone();
                        }
                    }

                    if is_enabled {
                        entity.insert(TabIndex(0));
                    } else {
                        entity.remove::<TabIndex>();

                        entity.world_scope(|world| {
                            if let Some(mut focus) = world.get_resource_mut::<InputFocus>()
                                && focus.get() == Some(id)
                            {
                                focus.clear();
                            }
                        });
                    }
                }),
                placeholder.map(|placeholder| {
                    spawn((
                        Text::new(placeholder),
                        TextColor(placeholder_color),
                        Node {
                            position_type: PositionType::Absolute,
                            ..Default::default()
                        },
                        // Never swallow a press meant for the editor underneath.
                        Pickable::IGNORE,
                    ))
                }),
            ))
    }
}

impl<'a> Modify<'a> for TextInput<'a> {
    fn modifier(&mut self) -> &mut Modifier<'a> {
        &mut self.modifier
    }
}
