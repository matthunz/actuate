// Timer UI example.

use actuate::prelude::*;
use bevy::prelude::*;

// Timer composable.
#[derive(Data)]
struct Timer;

impl Compose for Timer {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let current_time = use_mut(&cx, Time::default);

        // Use the `Time` resource from the ECS world, updating the `current_time`.
        use_world(&cx, move |time: Res<Time>| {
            SignalMut::set(current_time, *time)
        });

        // Spawn a `Text` component, updating it when this scope is re-composed.
        spawn(Text::new(format!("Elapsed: {:?}", current_time.elapsed())))
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ActuatePlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Spawn a composition with a `Timer`, adding it to the Actuate runtime.
    commands.spawn((Node::default(), Composition::new(Timer)));
}
