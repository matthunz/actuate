// Counter UI example.

use actuate::ecs::prelude::*;
use bevy::{prelude::*, winit::WinitSettings};

// Counter composable.
#[derive(Data)]
struct Counter {
    start: i32,
}

impl Compose for Counter {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let count = use_mut(&cx, || cx.me().start);

        material_ui((
            text::headline(format!("High five count: {}", count)),
            button(text::label("Up high")).on_click(move || SignalMut::update(count, |x| *x += 1)),
            button(text::label("Down low")).on_click(move || SignalMut::update(count, |x| *x -= 1)),
            if *count == 0 {
                Some(text::label("Gimme five!"))
            } else {
                None
            },
        ))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Spawn a composition with a `Counter`, adding it to the Actuate runtime.
    commands.spawn((
        Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            ..default()
        },
        Composition::new(Counter { start: 0 }),
    ));
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ActuatePlugin))
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, setup)
        .run();
}
