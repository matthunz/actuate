// HTTP UI example

use actuate::{executor::ExecutorContext, prelude::*};
use bevy::{prelude::*, winit::WinitSettings};
use serde::Deserialize;
use std::collections::HashMap;

// Dog breed composable.
#[derive(Data)]
struct Breed {
    name: String,
    families: Vec<String>,
}

impl Compose for Breed {
    fn compose(cx: Scope<Self>) -> impl Compose {
        container((
            text::headline(cx.me().name.to_owned()),
            compose::from_iter(cx.me().families.clone(), |family| {
                text::label(family.to_string())
            }),
        ))
    }
}

#[derive(Deserialize)]
struct Response {
    message: HashMap<String, Vec<String>>,
}

// Dog breed list composable.
#[derive(Data)]
struct BreedList;

impl Compose for BreedList {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let breeds = use_mut(&cx, HashMap::new);

        // Spawn a task that loads dog breeds from an HTTP API.
        use_task(&cx, move || async move {
            let json: Response = reqwest::get("https://dog.ceo/api/breeds/list/all")
                .await
                .unwrap()
                .json()
                .await
                .unwrap();

            SignalMut::set(breeds, json.message);
        });

        material_ui(
            // Render the currently loaded breeds.
            scroll_view(compose::from_iter((*breeds).clone(), |breed| Breed {
                name: breed.0.clone(),
                families: breed.1.clone(),
            }))
            .max_width(Val::Px(400.))
            .flex_gap(Val::Px(30.)),
        )
        .align_items(AlignItems::Center)
    }
}

#[derive(Data)]
struct Example;

impl Compose for Example {
    fn compose(cx: Scope<Self>) -> impl Compose {
        // Setup the Tokio executor.
        use_provider(&cx, ExecutorContext::default);

        BreedList
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ActuatePlugin))
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Spawn a composition with a `BreedList`, adding it to the Actuate runtime.
    commands.spawn((
        Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            ..default()
        },
        Composition::new(Example),
    ));
}
