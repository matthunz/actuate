use super::{NodeContext, use_dom_node};
use crate::{Scope, Signal, compose::Compose, data::Data, use_drop, use_provider, use_ref};
use alloc::rc::Rc;
use core::{cell::RefCell, mem};
use std::collections::BTreeSet;
use wasm_bindgen::prelude::*;

type HandlerFn<'a> = Rc<dyn Fn(web_sys::Event) + 'a>;

/// A slot holding the current handler for one event type.
///
/// The listener registered with the DOM reads through this slot, so the handler can be
/// replaced on every re-composition without touching the DOM, and cleared on drop so a
/// queued event can never call into a composable that no longer exists.
type HandlerSlot = Rc<RefCell<Option<HandlerFn<'static>>>>;

/// A DOM listener kept alive for as long as the element that registered it.
type EventClosure = Closure<dyn FnMut(web_sys::Event)>;

/// Create an [`Element`] composable that spawns the given HTML element when composed.
///
/// On re-composition, the element's attributes and event handlers are updated to the
/// latest provided values.
///
/// # Examples
///
/// ```ignore
/// use actuate::prelude::*;
///
/// #[derive(Data)]
/// struct Greeting {
///     name: String,
/// }
///
/// impl Compose for Greeting {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         element("h1")
///             .attr("class", "greeting")
///             .content(text(format!("Hello, {}!", cx.me().name)))
///     }
/// }
/// ```
pub fn element<'a>(tag: &'static str) -> Element<'a> {
    Element {
        tag,
        attrs: Vec::new(),
        handlers: Vec::new(),
        content: (),
    }
}

/// Composable to spawn an HTML element.
///
/// See [`element`] for more information.
#[derive(Clone)]
#[must_use = "Composables do nothing unless composed or returned from other composables."]
pub struct Element<'a, C = ()> {
    tag: &'static str,
    attrs: Vec<(&'static str, String)>,
    handlers: Vec<(&'static str, HandlerFn<'a>)>,
    content: C,
}

impl<'a, C> Element<'a, C> {
    /// Set the child content of this element.
    pub fn content<C2>(self, content: C2) -> Element<'a, C2> {
        Element {
            tag: self.tag,
            attrs: self.attrs,
            handlers: self.handlers,
            content,
        }
    }

    /// Set an attribute on this element.
    ///
    /// The attribute is re-applied on every re-composition.
    pub fn attr(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.attrs.push((name, value.into()));
        self
    }

    /// Set the `class` attribute on this element.
    pub fn class(self, value: impl Into<String>) -> Self {
        self.attr("class", value)
    }

    /// Set the `style` attribute on this element.
    pub fn style(self, value: impl Into<String>) -> Self {
        self.attr("style", value)
    }

    /// Set the `id` attribute on this element.
    pub fn id(self, value: impl Into<String>) -> Self {
        self.attr("id", value)
    }

    /// Add an event handler to this element.
    ///
    /// `event` is a DOM event name, such as `"click"` or `"input"`. The listener is
    /// registered once, on the initial composition, and the handler it calls is updated
    /// on every re-composition.
    pub fn on(mut self, event: &'static str, handler: impl Fn(web_sys::Event) + 'a) -> Self {
        self.handlers.push((event, Rc::new(handler)));
        self
    }
}

unsafe impl<C: Data> Data for Element<'_, C> {}

impl<C: Compose> Compose for Element<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let node = use_dom_node(&cx, |document| {
            document
                .create_element(cx.me().tag)
                .expect("failed to create element")
        });

        // Diff attributes against the last compose, so an unchanged attribute costs no
        // call into JS. Keyed by name rather than position, because attributes may be
        // applied conditionally and so are not guaranteed to keep a stable order.
        let applied: &RefCell<Vec<(&'static str, String)>> =
            use_ref(&cx, || RefCell::new(Vec::new()));
        {
            let mut applied = applied.borrow_mut();

            for (name, value) in &cx.me().attrs {
                match applied.iter_mut().find(|(last, _)| last == name) {
                    Some((_, last)) if last == value => {}
                    Some((_, last)) => {
                        last.clone_from(value);
                        let _ = node.set_attribute(name, value);
                    }
                    None => {
                        applied.push((name, value.clone()));
                        let _ = node.set_attribute(name, value);
                    }
                }
            }

            // Remove attributes that were set on a previous compose but are gone now.
            applied.retain(|(name, _)| {
                let is_set = cx.me().attrs.iter().any(|(current, _)| current == name);
                if !is_set {
                    let _ = node.remove_attribute(name);
                }
                is_set
            });
        }

        let slots: &RefCell<Vec<(&'static str, HandlerSlot)>> =
            use_ref(&cx, || RefCell::new(Vec::new()));

        // Closures are kept alive for as long as this scope, and dropped with it.
        let closures: &RefCell<Vec<EventClosure>> = use_ref(&cx, || RefCell::new(Vec::new()));

        for (event, handler) in &cx.me().handlers {
            let mut slots = slots.borrow_mut();

            let slot = match slots.iter().find(|(name, _)| name == event) {
                Some((_, slot)) => slot.clone(),
                None => {
                    let slot: HandlerSlot = Rc::new(RefCell::new(None));
                    slots.push((event, slot.clone()));

                    let listener = slot.clone();
                    let closure = EventClosure::new(move |event: web_sys::Event| {
                        // Clone out of the slot before calling, so a handler that
                        // triggers a re-compose cannot panic on a re-entrant borrow.
                        let handler = listener.borrow().clone();
                        if let Some(handler) = handler {
                            handler(event);
                        }
                    });

                    let _ = node
                        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
                    closures.borrow_mut().push(closure);

                    slot
                }
            };

            // Safety: the handler borrows from this composable, which outlives the slot.
            // `use_drop` below clears every slot before this scope is dropped, so the
            // listener can never call a handler that has outlived its data.
            let handler: HandlerFn<'static> = unsafe { mem::transmute(handler.clone()) };
            *slot.borrow_mut() = Some(handler);
        }

        use_drop(&cx, move || {
            for (_, slot) in slots.borrow().iter() {
                *slot.borrow_mut() = None;
            }
        });

        use_provider(&cx, || NodeContext {
            parent: node.clone().into(),
            keys: RefCell::new(BTreeSet::new()),
        });

        // Safety: the content is composed at exactly one place in the tree.
        unsafe { Signal::map_unchecked(cx.me(), |me| &me.content) }
    }
}

/// Create a text node composable.
///
/// The node's text is updated on every re-composition.
///
/// # Examples
///
/// ```ignore
/// use actuate::prelude::*;
///
/// let content = text("Hello, World!");
/// ```
pub fn text(content: impl Into<String>) -> Text {
    Text {
        content: content.into(),
    }
}

/// Composable for a DOM text node.
///
/// See [`text`] for more information.
#[derive(Clone)]
#[must_use = "Composables do nothing unless composed or returned from other composables."]
pub struct Text {
    content: String,
}

unsafe impl Data for Text {}

impl Compose for Text {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let node = use_dom_node(&cx, |document| document.create_text_node(&cx.me().content));

        // Update the text on re-composition, skipping the write when unchanged.
        if node.data() != cx.me().content {
            node.set_data(&cx.me().content);
        }
    }
}

macro_rules! impl_tags {
    ($($(#[$meta:meta])* $name:ident),* $(,)?) => {
        $(
            $(#[$meta])*
            pub fn $name<'a, C>(content: C) -> Element<'a, C> {
                element(stringify!($name)).content(content)
            }
        )*
    };
}

impl_tags!(
    /// Create an `<a>` element with the given content.
    a,
    /// Create a `<button>` element with the given content.
    button,
    /// Create a `<div>` element with the given content.
    div,
    /// Create an `<h1>` element with the given content.
    h1,
    /// Create an `<h2>` element with the given content.
    h2,
    /// Create an `<h3>` element with the given content.
    h3,
    /// Create a `<li>` element with the given content.
    li,
    /// Create a `<p>` element with the given content.
    p,
    /// Create a `<span>` element with the given content.
    span,
    /// Create a `<ul>` element with the given content.
    ul,
);

/// Create an `<input>` element.
pub fn input<'a>() -> Element<'a> {
    element("input")
}
