//! Web (DOM) integration.
//!
//! This backend mirrors the ECS backend: [`mount`] drives a [`Composer`] the way
//! `ActuatePlugin` drives one per Bevy `Update`, and [`Element`] attaches a DOM node to
//! its parent the way `Spawn` attaches an entity, including the sibling-ordering rules
//! that keep children in composition order.
//!
//! Because this module and the `material` module both define `text`, `button` and
//! `Composition`, the web items are not re-exported from [`crate::prelude`]. Import them
//! from here instead:
//!
//! ```ignore
//! use actuate::{prelude::*, web::*};
//! ```
//!
//! If the `material` feature is also enabled, import the web items by name so they take
//! precedence over the glob:
//!
//! ```ignore
//! use actuate::{prelude::*, web::{button, div, text}};
//! ```
//!
//! # Async tasks
//!
//! Use [`use_local_task`](crate::use_local_task) rather than `use_task`. The `executor`
//! feature takes a blocking lock when applying updates, which cannot block on the wasm
//! main thread.

use crate::{
    ScopeState, Signal,
    compose::Compose,
    composer::{Composer, Pending, Runtime},
    data::Data,
    use_context, use_drop, use_provider, use_ref,
};
use alloc::{rc::Rc, sync::Arc, task::Wake};
use core::{
    cell::{Cell, RefCell},
    task::{Context, Poll, Waker},
};
use slotmap::{DefaultKey, SlotMap};
use std::collections::BTreeSet;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Node};

mod element;
pub use self::element::{
    Element, Text, a, button, div, element, h1, h2, h3, input, li, p, span, text, ul,
};

/// Web runtime context.
///
/// This mirrors the ECS backend's runtime context, caching the handles that
/// composables need rather than re-fetching them from JS on every compose.
#[derive(Clone)]
struct RuntimeContext {
    document: Document,
}

impl RuntimeContext {
    /// Get the current [`RuntimeContext`].
    ///
    /// # Panics
    /// Panics if called outside of a composition.
    fn current() -> Self {
        RUNTIME_CONTEXT.with(|cell| {
            let cell_ref = cell.borrow();
            let Some(rt) = cell_ref.as_ref() else {
                panic!("Must be called from within a composable.")
            };
            rt.clone()
        })
    }
}

thread_local! {
    static RUNTIME_CONTEXT: RefCell<Option<RuntimeContext>> = const { RefCell::new(None) };
}

/// Use the current [`Document`].
///
/// # Examples
///
/// ```ignore
/// use actuate::prelude::*;
///
/// #[derive(Data)]
/// struct Title;
///
/// impl Compose for Title {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         let document = use_document(&cx);
///         document.set_title("Actuate");
///     }
/// }
/// ```
pub fn use_document(cx: ScopeState<'_>) -> &Document {
    &use_ref(cx, RuntimeContext::current).document
}

/// Context provided to child composables, holding the DOM node they attach to.
///
/// `keys` tracks the tree positions of the children attached to `parent`, so that
/// siblings are inserted in composition order even though they may compose out of order.
/// This mirrors `SpawnContext` in the ECS backend.
struct NodeContext {
    parent: Node,
    keys: RefCell<BTreeSet<Pending>>,
}

/// Insert `child` into `parent` at `idx`, appending if `idx` is past the end.
fn insert_child_at(parent: &Node, child: &Node, idx: usize) {
    let children = parent.child_nodes();
    match children.get(idx as u32) {
        Some(before) => {
            let _ = parent.insert_before(child, Some(&before));
        }
        None => {
            let _ = parent.append_child(child);
        }
    }
}

/// Create a DOM node, attach it to the parent composable's node in the correct sibling
/// position, and remove it when this scope is dropped.
///
/// `make_node` is called once, on the initial composition.
fn use_dom_node<'a, N>(cx: ScopeState<'a>, make_node: impl FnOnce(&Document) -> N) -> &'a N
where
    N: AsRef<Node> + 'static,
{
    let rt = Runtime::current();
    let node_cx = use_context::<NodeContext>(cx);
    let key = use_ref(cx, || rt.pending(rt.current_key.get()));

    let node = use_ref(cx, || {
        let node = make_node(&RuntimeContext::current().document);

        if let Ok(node_cx) = &node_cx {
            node_cx.keys.borrow_mut().insert(key.clone());

            let idx = node_cx
                .keys
                .borrow()
                .iter()
                .position(|pending| pending.key == rt.current_key.get());

            if let Some(idx) = idx {
                insert_child_at(&node_cx.parent, node.as_ref(), idx);
            }
        }

        node
    });

    use_drop(cx, move || {
        if let Ok(node_cx) = &node_cx {
            node_cx.keys.borrow_mut().remove(key);
        }

        let node: &Node = node.as_ref();
        if let Some(parent) = node.parent_node() {
            let _ = parent.remove_child(node);
        }
    });

    node
}

/// Root composable, providing the mount point to its children.
///
/// This mirrors `CompositionContent` in the ECS backend.
#[derive(Data)]
#[actuate(path = "crate")]
struct Root<C> {
    content: C,
    parent: Node,
}

impl<C: Compose> Compose for Root<C> {
    fn compose(cx: crate::Scope<Self>) -> impl Compose {
        use_provider(&cx, || NodeContext {
            parent: cx.me().parent.clone(),
            keys: RefCell::new(BTreeSet::new()),
        });

        // Safety: the content is composed at exactly one place in the tree.
        unsafe { Signal::map_unchecked(cx.me(), |me| &me.content) }
    }
}

/// The `requestAnimationFrame` callback driving a composition.
///
/// Held in an `Rc` so the callback can re-schedule itself through a [`Weak`] reference,
/// and in an `Option` because the callback is only built once the cell exists.
type FrameCallback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

/// Schedules animation frames for one composition.
struct Scheduler {
    callback: FrameCallback,

    /// The pending animation frame, if one is scheduled.
    frame: Cell<Option<i32>>,
}

impl Scheduler {
    /// Schedule a frame, if one is not already pending.
    ///
    /// Coalescing here is what makes a burst of updates from a single event handler cost
    /// one frame rather than one frame per update.
    fn schedule(&self) {
        if self.frame.get().is_some() {
            return;
        }

        if let Some(callback) = self.callback.borrow().as_ref() {
            self.frame.set(Some(request_animation_frame(callback)));
        }
    }

    /// Cancel the pending frame, if any.
    fn cancel(&self) {
        if let Some(frame) = self.frame.take()
            && let Some(window) = web_sys::window()
        {
            window.cancel_animation_frame(frame).ok();
        }
    }
}

thread_local! {
    /// Schedulers for the compositions running on this thread.
    ///
    /// [`FrameWaker`] reaches its scheduler through this rather than holding it, so that
    /// the waker itself stays `Send + Sync` without any unsafe impls.
    static SCHEDULERS: RefCell<SlotMap<DefaultKey, Rc<Scheduler>>> =
        RefCell::new(SlotMap::new());
}

/// Waker that schedules an animation frame for its composition.
///
/// This holds only a key: `Wake` requires `Send + Sync`, which neither `Rc` nor `Window`
/// satisfies. A wake from another thread finds no scheduler in that thread's registry and
/// is dropped, which stalls the composition rather than causing undefined behavior. On
/// wasm without threads, every wake arrives on the thread that owns the scheduler.
struct FrameWaker {
    key: DefaultKey,
}

impl Wake for FrameWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        SCHEDULERS.with(|schedulers| {
            let scheduler = schedulers.borrow().get(self.key).cloned();
            if let Some(scheduler) = scheduler {
                scheduler.schedule();
            }
        });
    }
}

/// A composition mounted into the DOM.
///
/// Dropping this stops the composition and removes the content it spawned.
#[must_use = "A `Composition` stops running when dropped."]
pub struct Composition {
    key: DefaultKey,
}

impl Drop for Composition {
    fn drop(&mut self) {
        let scheduler = SCHEDULERS.with(|schedulers| schedulers.borrow_mut().remove(self.key));

        if let Some(scheduler) = scheduler {
            // Cancel before the callback is freed, so the browser can never invoke a
            // closure that no longer exists.
            scheduler.cancel();
            scheduler.callback.borrow_mut().take();
        }
    }
}

/// Mount `content` into the given DOM node and begin composing.
///
/// The composition is driven on demand: a frame is scheduled when the composer is woken
/// by a state update or a task, and when a pass leaves work still queued. An idle
/// composition schedules no frames at all.
///
/// Because this is built on `requestAnimationFrame`, a composition in a hidden tab is
/// paused until the tab is visible again.
///
/// # Examples
///
/// ```ignore
/// use actuate::prelude::*;
///
/// #[derive(Data)]
/// struct App;
///
/// impl Compose for App {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         let count = use_mut(&cx, || 0);
///
///         div((
///             text(format!("High five count: {}", count)),
///             button(text("Up high")).on("click", move |_| SignalMut::update(count, |x| *x += 1)),
///             button(text("Down low")).on("click", move |_| SignalMut::update(count, |x| *x -= 1)),
///         ))
///     }
/// }
///
/// let _composition = actuate::web::mount_to_body(App);
/// ```
pub fn mount(parent: impl AsRef<Node>, content: impl Compose + 'static) -> Composition {
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("window has no document");

    RUNTIME_CONTEXT.with(|cell| {
        *cell.borrow_mut() = Some(RuntimeContext {
            document: document.clone(),
        });
    });

    let mut composer = Composer::new(Root {
        content,
        parent: parent.as_ref().clone(),
    });

    let scheduler = Rc::new(Scheduler {
        callback: Rc::new(RefCell::new(None)),
        frame: Cell::new(None),
    });

    let key = SCHEDULERS.with(|schedulers| schedulers.borrow_mut().insert(scheduler.clone()));
    let waker = Waker::from(Arc::new(FrameWaker { key }));

    // A `Weak` reference, so the callback does not keep itself alive.
    let handle = Rc::downgrade(&scheduler);

    *scheduler.callback.borrow_mut() = Some(Closure::new(move || {
        let Some(scheduler) = handle.upgrade() else {
            return;
        };

        // This frame is no longer pending, so work queued during the pass can schedule
        // the next one.
        scheduler.frame.set(None);

        let mut cx = Context::from_waker(&waker);

        // TODO handle composition error.
        if let Poll::Ready(Err(error)) = composer.poll_compose(&mut cx) {
            web_sys::console::error_1(&format!("Actuate composition error: {error}").into());
        }

        // Applying a queued update can itself queue a recomposition, and that happens on
        // the way out of the pass above, after the waker has already fired. So the queues
        // are checked directly rather than relying on `Poll::Pending` to mean "idle".
        if composer.is_ready() {
            scheduler.schedule();
        }
    }));

    scheduler.schedule();

    Composition { key }
}

/// Mount `content` into the document body and begin composing.
///
/// For more see [`mount`].
pub fn mount_to_body(content: impl Compose + 'static) -> Composition {
    let body = web_sys::window()
        .expect("no global `window` exists")
        .document()
        .expect("window has no document")
        .body()
        .expect("document has no body");

    mount(body, content)
}

/// Schedule `f` for the next animation frame, returning its cancellation handle.
fn request_animation_frame(f: &Closure<dyn FnMut()>) -> i32 {
    web_sys::window()
        .expect("no global `window` exists")
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("failed to request an animation frame")
}
