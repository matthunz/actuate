// Counter UI example.

use actuate::ecs::prelude::*;
use bevy::prelude::*;

// Counter composable.
#[derive(Data)]
struct Example;

impl Compose for Example {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let is_shown = use_mut(&cx, || true);

        radio_button()
            .is_enabled(*is_shown)
            .on_click(move || SignalMut::update(is_shown, |x| *x = !*x))
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Spawn a composition with a `Counter`, adding it to the Actuate runtime.
    commands.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.),
            ..default()
        },
        Composition::new(Example),
    ));
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ActuatePlugin))
        .add_systems(Startup, setup)
        .run();
}
