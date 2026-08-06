//! Tests for the controlled value binding of the `text_input` composable.
// Bevy's `App` isn't supported under Miri.
#![cfg(all(feature = "ui", not(miri)))]

use actuate::ecs::prelude::*;
use bevy::{
    input_focus::tab_navigation::TabIndex,
    prelude::*,
    text::{EditableText, TextColor},
};
use std::sync::{Arc, Mutex};

/// State driving the composition from outside the composer.
#[derive(Resource)]
struct Config {
    value: String,
    is_enabled: bool,
}

/// Every value the input has reported, in order.
#[derive(Default, Resource, Clone)]
struct Changes(Arc<Mutex<Vec<String>>>);

#[derive(Data)]
struct Root;

impl Compose for Root {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let value = use_mut(&cx, String::new);
        let is_enabled = use_mut(&cx, || true);

        use_world(&cx, move |config: Res<Config>| {
            SignalMut::set_if_neq(value, config.value.clone());
            SignalMut::set_if_neq(is_enabled, config.is_enabled);
        });

        // Share the recorder with the test, which reads it straight off the resource.
        let changes = use_world_once(&cx, |changes: Res<Changes>| changes.clone()).clone();

        text_input()
            .value(&**value)
            .is_enabled(*is_enabled)
            .on_change(move |next| changes.0.lock().unwrap().push(next))
    }
}

fn setup() -> App {
    let mut app = App::new();
    app.add_plugins(ActuatePlugin)
        .init_resource::<Changes>()
        .insert_resource(Config {
            value: String::new(),
            is_enabled: true,
        });

    app.world_mut().spawn(Composition::new(Root));

    settle(&mut app);
    app
}

/// Update until the composition settles.
///
/// A change to the world takes two updates to reach the entity: one to queue the
/// signal update, and one to re-compose with it.
fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

/// Get the single entity holding an `EditableText` component.
fn editor(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<EditableText>>();
    let entities: Vec<_> = query.iter(world).collect();

    assert_eq!(entities.len(), 1, "expected exactly one editable entity");
    entities[0]
}

/// Read the editor's current contents.
fn text(app: &mut App) -> String {
    let entity = editor(app);
    app.world()
        .entity(entity)
        .get::<EditableText>()
        .unwrap()
        .value()
        .to_string()
}

/// Type into the editor, standing in for keyboard input.
///
/// This writes through `PlainEditor` directly rather than queueing a `TextEdit`, because
/// `apply_text_edits` is only registered by Bevy's `TextPlugin`.
fn type_text(app: &mut App, value: &str) {
    let entity = editor(app);
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<EditableText>()
        .unwrap()
        .editor
        .set_text(value);
}

/// Read the editor's current selection, as a range over the text.
fn selected_range(app: &App, entity: Entity) -> std::ops::Range<usize> {
    app.world()
        .entity(entity)
        .get::<EditableText>()
        .unwrap()
        .editor
        .raw_selection()
        .text_range()
}

fn changes(app: &App) -> Vec<String> {
    app.world().resource::<Changes>().0.lock().unwrap().clone()
}

#[test]
fn it_spawns_with_the_initial_value() {
    let mut app = App::new();
    app.add_plugins(ActuatePlugin)
        .init_resource::<Changes>()
        .insert_resource(Config {
            value: "hello".into(),
            is_enabled: true,
        });

    app.world_mut().spawn(Composition::new(Root));
    settle(&mut app);

    assert_eq!(text(&mut app), "hello");
}

#[test]
fn it_syncs_external_value_changes() {
    let mut app = setup();
    let entity = editor(&mut app);

    app.world_mut().resource_mut::<Config>().value = "typed".into();
    settle(&mut app);

    assert_eq!(editor(&mut app), entity, "the entity should be reused");
    assert_eq!(text(&mut app), "typed");
}

#[test]
fn it_does_not_clobber_the_editor_on_unrelated_recomposes() {
    let mut app = setup();

    // Simulate a keystroke that has already round-tripped through the caller.
    type_text(&mut app, "abc");
    app.world_mut().resource_mut::<Config>().value = "abc".into();
    settle(&mut app);

    let entity = editor(&mut app);
    let selection = selected_range(&app, entity);

    // A field the composable never writes. If `EditableText` were part of the spawned bundle,
    // re-composing would insert a fresh one and reset this to its default, taking the live
    // buffer, caret, and selection with it.
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<EditableText>()
        .unwrap()
        .cursor_width = 0.75;

    // Re-compose for a reason that has nothing to do with the value.
    app.world_mut().resource_mut::<Config>().is_enabled = false;
    settle(&mut app);

    assert_eq!(text(&mut app), "abc", "the buffer should survive");
    assert_eq!(
        selected_range(&app, entity),
        selection,
        "the selection should survive"
    );
    assert_eq!(
        app.world()
            .entity(entity)
            .get::<EditableText>()
            .unwrap()
            .cursor_width,
        0.75,
        "`EditableText` should not be re-inserted on re-compose"
    );
}

#[test]
fn it_reports_edits_through_on_change() {
    let mut app = setup();

    type_text(&mut app, "hi");
    settle(&mut app);

    assert_eq!(changes(&app), vec!["hi".to_string()]);

    // Settling again with nothing edited must not report the same value twice.
    settle(&mut app);
    assert_eq!(changes(&app), vec!["hi".to_string()]);
}

#[test]
fn it_reverts_an_edit_the_caller_does_not_accept() {
    let mut app = setup();

    // `Config::value` is the caller's state, and it never accepts this edit.
    type_text(&mut app, "rejected");
    settle(&mut app);

    assert_eq!(changes(&app), vec!["rejected".to_string()]);
    assert_eq!(
        text(&mut app),
        "",
        "an edit the caller doesn't write back should revert"
    );
}

/// Bevy's `TextColor` defaults to white, which is invisible against the theme's background, so
/// the input has to take its color from the theme instead.
#[cfg(feature = "material")]
#[test]
fn it_takes_its_color_from_the_theme() {
    #[derive(Data)]
    struct Themed;

    impl Compose for Themed {
        fn compose(_cx: Scope<Self>) -> impl Compose {
            material_ui(text_input().placeholder("Name"))
        }
    }

    let mut app = App::new();
    app.add_plugins(ActuatePlugin);

    app.world_mut().spawn(Composition::new(Themed));
    settle(&mut app);

    let entity = editor(&mut app);
    assert_eq!(
        app.world().entity(entity).get::<TextColor>().unwrap().0,
        Theme::default().colors.text,
        "the editor should use the theme's text color"
    );

    let placeholder = app
        .world_mut()
        .query_filtered::<&TextColor, With<Text>>()
        .iter(app.world())
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(placeholder.len(), 1, "expected exactly one placeholder");
    assert!(
        placeholder[0].0.alpha() < Theme::default().colors.text.alpha(),
        "the placeholder should be dimmed relative to the text"
    );
}

#[test]
fn it_removes_tab_index_when_disabled() {
    let mut app = setup();

    let entity = editor(&mut app);
    assert!(app.world().entity(entity).contains::<TabIndex>());

    app.world_mut().resource_mut::<Config>().is_enabled = false;
    settle(&mut app);

    assert_eq!(editor(&mut app), entity, "the entity should be reused");
    assert!(
        !app.world().entity(entity).contains::<TabIndex>(),
        "a disabled input should drop out of tab navigation"
    );
}
