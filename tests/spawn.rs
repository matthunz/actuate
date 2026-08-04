//! Tests for the entity and component lifecycle of the `spawn` composable.
// Bevy's `App` isn't supported under Miri.
#![cfg(all(feature = "ecs", not(miri)))]

use actuate::prelude::*;
use bevy::prelude::*;

/// State driving the composition from outside the composer.
#[derive(Resource)]
struct Config {
    has_inventory: bool,
}

#[derive(Component, Clone)]
struct Player;

#[derive(Component, Clone)]
struct Inventory;

/// A component added by something other than the composition, like another plugin.
#[derive(Component)]
struct TargetedEnemy;

/// Spawns a `Player`, with an `Inventory` only while `Config::has_inventory` is set.
#[derive(Data)]
struct Root;

impl Compose for Root {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let has_inventory = use_mut(&cx, || false);

        use_world(&cx, move |config: Res<Config>| {
            SignalMut::set_if_neq(has_inventory, config.has_inventory)
        });

        if *has_inventory {
            spawn((Player, Inventory))
        } else {
            spawn(Player)
        }
    }
}

fn setup(has_inventory: bool) -> App {
    let mut app = App::new();
    app.add_plugins(ActuatePlugin)
        .insert_resource(Config { has_inventory });

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

/// Get the single entity holding a `Player` component.
fn player(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<Player>>();
    let entities: Vec<_> = query.iter(world).collect();

    assert_eq!(entities.len(), 1, "expected exactly one player entity");
    entities[0]
}

#[test]
fn it_removes_components_that_are_no_longer_composed() {
    let mut app = setup(true);

    let entity = player(&mut app);
    assert!(app.world().entity(entity).contains::<Inventory>());

    app.world_mut().resource_mut::<Config>().has_inventory = false;
    settle(&mut app);

    assert_eq!(player(&mut app), entity, "the entity should be reused");
    assert!(
        !app.world().entity(entity).contains::<Inventory>(),
        "`Inventory` should be removed once it's no longer composed"
    );
}

#[test]
fn it_inserts_components_that_are_composed_again() {
    let mut app = setup(false);

    let entity = player(&mut app);
    assert!(!app.world().entity(entity).contains::<Inventory>());

    app.world_mut().resource_mut::<Config>().has_inventory = true;
    settle(&mut app);

    assert_eq!(player(&mut app), entity, "the entity should be reused");
    assert!(app.world().entity(entity).contains::<Inventory>());
}

#[test]
fn it_preserves_components_inserted_externally() {
    let mut app = setup(true);

    let entity = player(&mut app);
    app.world_mut().entity_mut(entity).insert(TargetedEnemy);

    app.world_mut().resource_mut::<Config>().has_inventory = false;
    settle(&mut app);

    assert!(!app.world().entity(entity).contains::<Inventory>());
    assert!(
        app.world().entity(entity).contains::<TargetedEnemy>(),
        "components this composable didn't insert should be left alone"
    );
}

/// State recorded by the `on_spawn` and `on_insert` handlers.
#[derive(Default, Resource)]
struct Counts {
    spawns: usize,
    inserts: usize,
}

#[derive(Data)]
struct CountingRoot;

impl Compose for CountingRoot {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let has_inventory = use_mut(&cx, || false);

        use_world(&cx, move |config: Res<Config>| {
            SignalMut::set_if_neq(has_inventory, config.has_inventory)
        });

        let bundle = if *has_inventory {
            spawn((Player, Inventory))
        } else {
            spawn(Player)
        };

        bundle
            .on_spawn(|mut entity| {
                entity.world_scope(|world| world.resource_mut::<Counts>().spawns += 1)
            })
            .on_insert(|mut entity| {
                entity.world_scope(|world| world.resource_mut::<Counts>().inserts += 1)
            })
    }
}

#[test]
fn it_only_calls_on_spawn_once() {
    let mut app = App::new();
    app.add_plugins(ActuatePlugin)
        .insert_resource(Config {
            has_inventory: false,
        })
        .init_resource::<Counts>();

    app.world_mut().spawn(Composition::new(CountingRoot));
    settle(&mut app);

    app.world_mut().resource_mut::<Config>().has_inventory = true;
    settle(&mut app);

    let counts = app.world().resource::<Counts>();
    assert_eq!(counts.spawns, 1, "`on_spawn` should only run for the spawn");
    assert!(
        counts.inserts > 1,
        "`on_insert` should run for every composition, got {}",
        counts.inserts
    );
}
