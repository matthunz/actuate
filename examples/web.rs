// Web counter example.
//
// Build for the browser with:
//   cargo build --example web --target wasm32-unknown-unknown --features web
//
// Then generate bindings with `wasm-bindgen` and serve the result alongside an HTML
// page that imports the generated module.

// The web items are imported by name rather than with a glob, so they take precedence
// over the `material` module's `button`/`text` when both features are enabled.
use actuate::{
    prelude::*,
    web::{button, div, h1, mount_to_body, p, text},
};

// Counter composable.
#[derive(Data)]
struct Counter {
    start: i32,
}

impl Compose for Counter {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let count = use_mut(&cx, || cx.me().start);

        div((
            h1(text(format!("High five count: {}", count))),
            button(text("Up high")).on("click", move |_| SignalMut::update(count, |x| *x += 1)),
            button(text("Down low")).on("click", move |_| SignalMut::update(count, |x| *x -= 1)),
            if *count == 0 {
                Some(p(text("Gimme five!")))
            } else {
                None
            },
        ))
        .class("counter")
    }
}

fn main() {
    // Mount the composition into the document body, where it runs for the lifetime
    // of the page.
    std::mem::forget(mount_to_body(Counter { start: 0 }));
}
