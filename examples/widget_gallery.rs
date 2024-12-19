// Counter UI example.

use actuate::prelude::*;
use bevy::{asset::io::SeekForwardFuture, prelude::*};

// Counter composable.
#[derive(Data)]
struct WidgetGallery;

impl Compose for WidgetGallery {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let is_shown = use_mut(&cx, || true);
        // let selected_value = use_mut(&cx, || 5.);

        material_ui((
            radio_button().is_enabled(*is_shown),
            switch()
                .is_enabled(*is_shown)
                .on_click(move || SignalMut::update(is_shown, |x| *x = !*x)),
            // slider().current(*selected_value),
        ))
        .width(Val::Vw(100.))
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());

    // Spawn a composition with a `Counter`, adding it to the Actuate runtime.
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
