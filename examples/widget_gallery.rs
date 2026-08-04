// Widget gallery UI example.

use actuate::prelude::*;
use bevy::prelude::*;

// Widget gallery composable.
#[derive(Data)]
struct WidgetGallery;

impl Compose for WidgetGallery {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let is_on = use_mut(&cx, || false);

        material_ui((
            radio_button().is_enabled(*is_on),
            switch()
                .is_checked(*is_on)
                .on_click(move || SignalMut::update(is_on, |x| *x = !*x)),
            // Disabled switches, shown in both states.
            switch().is_enabled(false),
            switch().is_checked(true).is_enabled(false),
        ))
        .width(Val::Vw(100.))
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Spawn a composition with a `WidgetGallery`, adding it to the Actuate runtime.
    commands.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.),
            ..default()
        },
        Composition::new(WidgetGallery),
    ));
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ActuatePlugin))
        .add_systems(Startup, setup)
        .run();
}
