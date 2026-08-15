//! Tests for the DOM node lifecycle and sibling ordering of the web backend.
//!
//! These drive a real browser DOM, so they only build for wasm. Run them with:
//!
//! ```sh
//! wasm-pack test --headless --chrome -- --features web
//! ```
#![cfg(all(target_arch = "wasm32", feature = "web"))]

use actuate::web::prelude::*;
use js_sys::Promise;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("no window")
        .document()
        .expect("no document")
}

/// A fresh element in the document for one test to mount into.
///
/// Every test gets its own, so compositions can't observe each other's nodes.
fn container() -> web_sys::Element {
    let document = document();
    let element = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&element).unwrap();
    element
}

/// Await one animation frame, which is what drives a mounted composition.
async fn frame() {
    let promise = Promise::new(&mut |resolve, _reject| {
        web_sys::window()
            .unwrap()
            .request_animation_frame(&resolve)
            .unwrap();
    });

    JsFuture::from(promise).await.unwrap();
}

/// Await enough frames for a queued update to reach the DOM.
///
/// An update takes more than one pass: one to apply it, and one to re-compose with the
/// new value. This mirrors `settle` in `tests/spawn.rs`.
async fn settle() {
    for _ in 0..5 {
        frame().await;
    }
}

/// Click the element matching `selector`, as a user would.
fn click(parent: &web_sys::Element, selector: &str) {
    parent
        .query_selector(selector)
        .unwrap()
        .unwrap_or_else(|| panic!("no element matching `{selector}`"))
        .dyn_into::<web_sys::HtmlElement>()
        .unwrap()
        .click();
}

/// Three static siblings.
#[derive(Data)]
struct Siblings;

impl Compose for Siblings {
    fn compose(_cx: Scope<Self>) -> impl Compose {
        (h1(text("one")), p(text("two")), span(text("three")))
    }
}

#[wasm_bindgen_test]
async fn it_mounts_children_in_composition_order() {
    let parent = container();
    let _composition = mount(&parent, Siblings);
    settle().await;

    assert_eq!(
        parent.inner_html(),
        "<h1>one</h1><p>two</p><span>three</span>"
    );
}

/// A middle child that only appears once toggled, so it is created *after* the sibling
/// that follows it is already in the DOM.
#[derive(Data)]
struct Middle;

impl Compose for Middle {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let is_shown = use_mut(&cx, || false);

        (
            h1(text("first")),
            if *is_shown {
                Some(p(text("middle")))
            } else {
                None
            },
            span(text("last")),
            button(text("toggle"))
                .id("toggle")
                .on("click", move |_| {
                    SignalMut::update(is_shown, |is_shown| *is_shown = !*is_shown)
                }),
        )
    }
}

#[wasm_bindgen_test]
async fn it_inserts_a_late_child_between_its_siblings() {
    let parent = container();
    let _composition = mount(&parent, Middle);
    settle().await;

    assert_eq!(
        parent.inner_html(),
        r#"<h1>first</h1><span>last</span><button id="toggle">toggle</button>"#
    );

    click(&parent, "#toggle");
    settle().await;

    // The `<p>` is created last but must land in composition order, between the
    // siblings that already exist.
    assert_eq!(
        parent.inner_html(),
        r#"<h1>first</h1><p>middle</p><span>last</span><button id="toggle">toggle</button>"#
    );
}

#[wasm_bindgen_test]
async fn it_removes_a_node_when_its_scope_drops() {
    let parent = container();
    let _composition = mount(&parent, Middle);
    settle().await;

    click(&parent, "#toggle");
    settle().await;
    assert!(parent.inner_html().contains("<p>middle</p>"));

    click(&parent, "#toggle");
    settle().await;

    assert_eq!(
        parent.inner_html(),
        r#"<h1>first</h1><span>last</span><button id="toggle">toggle</button>"#,
        "the node should be removed once it's no longer composed"
    );
}

#[derive(Data)]
struct Item<'a> {
    label: Signal<'a, String>,
}

impl Compose for Item<'_> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        li(text(cx.me().label.to_string()))
    }
}

/// A list whose length is driven from a button, over `from_iter`.
#[derive(Data)]
struct List;

impl Compose for List {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let count = use_mut(&cx, || 2);

        let labels: Vec<String> = (0..*count).map(|idx| format!("item {idx}")).collect();

        (
            ul(compose::from_iter(labels, |label| Item { label })),
            button(text("add"))
                .id("add")
                .on("click", move |_| SignalMut::update(count, |c| *c += 1)),
        )
    }
}

#[wasm_bindgen_test]
async fn it_updates_children_from_an_iterator() {
    let parent = container();
    let _composition = mount(&parent, List);
    settle().await;

    let list = parent.query_selector("ul").unwrap().unwrap();
    assert_eq!(list.inner_html(), "<li>item 0</li><li>item 1</li>");

    click(&parent, "#add");
    settle().await;

    assert_eq!(
        list.inner_html(),
        "<li>item 0</li><li>item 1</li><li>item 2</li>"
    );
}

/// A counter, to check a text node is updated in place rather than replaced.
#[derive(Data)]
struct Counter;

impl Compose for Counter {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let count = use_mut(&cx, || 0);

        div((
            p(text(format!("count: {}", count))),
            button(text("up"))
                .id("up")
                .on("click", move |_| SignalMut::update(count, |c| *c += 1)),
        ))
    }
}

#[wasm_bindgen_test]
async fn it_updates_text_in_place() {
    let parent = container();
    let _composition = mount(&parent, Counter);
    settle().await;

    let text_node = parent.query_selector("p").unwrap().unwrap().first_child();
    assert_eq!(parent.query_selector("p").unwrap().unwrap().text_content(), Some("count: 0".into()));

    click(&parent, "#up");
    settle().await;

    assert_eq!(
        parent.query_selector("p").unwrap().unwrap().text_content(),
        Some("count: 1".into())
    );
    assert!(
        text_node
            .as_ref()
            .is_some_and(|node| node.is_same_node(
                parent.query_selector("p").unwrap().unwrap().first_child().as_ref()
            )),
        "the text node should be updated, not replaced"
    );
}

#[wasm_bindgen_test]
async fn it_mounts_into_a_parent_that_already_has_children() {
    let parent = container();
    parent.set_inner_html("<footer>existing</footer>");

    let _composition = mount(&parent, Siblings);
    settle().await;

    // Actuate indexes its own children, so its content occupies the front of the parent
    // and nodes it doesn't own are left after it, untouched.
    assert_eq!(
        parent.inner_html(),
        "<h1>one</h1><p>two</p><span>three</span><footer>existing</footer>"
    );
}

#[wasm_bindgen_test]
async fn dropping_the_composition_removes_its_content() {
    let parent = container();
    parent.set_inner_html("<footer>existing</footer>");

    let composition = mount(&parent, Siblings);
    settle().await;
    assert!(parent.inner_html().contains("<h1>one</h1>"));

    drop(composition);
    settle().await;

    assert_eq!(
        parent.inner_html(),
        "<footer>existing</footer>",
        "dropping the composition should remove what it spawned, and nothing else"
    );
}
