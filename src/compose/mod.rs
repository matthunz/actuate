use crate::{
    composer::{ComposePtr, Node, Runtime},
    data::Data,
    use_context, use_ref, Scope, ScopeData, ScopeState,
};
use alloc::borrow::Cow;
use alloc::rc::Rc;
use core::{
    any::TypeId,
    cell::{Cell, RefCell, UnsafeCell},
    fmt, mem,
};
use slotmap::{DefaultKey, SlotMap};

mod catch;
pub use self::catch::{catch, Catch};

mod dyn_compose;
pub use self::dyn_compose::{dyn_compose, DynCompose};

mod from_fn;
pub use self::from_fn::{from_fn, FromFn};

mod from_iter;
pub use self::from_iter::{from_iter, FromIter};

mod memo;
pub use self::memo::{memo, Memo};

/// A composable function.
///
/// For a dynamically-typed composable, see [`DynCompose`].
///
/// Composables are the building blocks of reactivity in Actuate.
/// A composable is essentially a function that is re-run whenever its state (or its parent state) is changed.
/// Composables may return one or more children, that run after their parent.
///
/// When a composable is re-run, we call that "recomposition".
/// For example, on the initial composition, hooks may initialize their state.
/// Then on recomposition, hooks update their state from the last set value.
///
/// Triggering a state update will recompose each parent, and then each child,
/// until either a [`Memo`] is reached or the composition is complete.
///
/// [`Memo`] is special in that it will only recompose in two cases:
/// 1. It's provided dependencies have changed (see [`memo()`] for more)
/// 2. Its own state has changed, which will then trigger the above parent-to-child process for its children.
#[must_use = "Composables do nothing unless composed or returned from other composables."]
pub trait Compose: Data {
    /// Compose this function.
    fn compose(cx: Scope<Self>) -> impl Compose;

    #[doc(hidden)]
    fn name() -> Option<Cow<'static, str>> {
        let name = core::any::type_name::<Self>();
        Some(
            name.split('<')
                .next()
                .unwrap_or(name)
                .split("::")
                .last()
                .unwrap_or(name)
                .into(),
        )
    }
}

impl Compose for () {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let _ = cx;
    }
}

impl<C: Compose> Compose for Option<C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let child_key = use_ref(&cx, || Cell::new(None));

        let rt = Runtime::current();
        let mut nodes = rt.nodes.borrow_mut();

        if let Some(content) = &*cx.me() {
            if let Some(key) = child_key.get() {
                let last = nodes.get_mut(key).unwrap();

                let ptr = content as *const dyn AnyCompose;
                let ptr: *const dyn AnyCompose = unsafe { mem::transmute(ptr) };

                *last.compose.borrow_mut() = ComposePtr::Ptr(ptr);

                drop(nodes);

                rt.queue(key);
            } else {
                let ptr: *const dyn AnyCompose =
                    unsafe { mem::transmute(content as *const dyn AnyCompose) };
                let key = nodes.insert(Rc::new(Node {
                    compose: RefCell::new(crate::composer::ComposePtr::Ptr(ptr)),
                    scope: ScopeData::default(),
                    parent: Some(rt.current_key.get()),
                    children: RefCell::new(Vec::new()),
                    child_idx: 0,
                }));
                child_key.set(Some(key));

                nodes
                    .get(rt.current_key.get())
                    .unwrap()
                    .children
                    .borrow_mut()
                    .push(key);

                let child_state = &nodes[key].scope;

                *child_state.contexts.borrow_mut() = cx.contexts.borrow().clone();
                child_state
                    .contexts
                    .borrow_mut()
                    .values
                    .extend(cx.child_contexts.borrow().values.clone());

                drop(nodes);

                rt.queue(key);
            }
        } else if let Some(key) = child_key.get() {
            child_key.set(None);

            drop_node(&mut nodes, key);
        }
    }
}

// TODO replace with non-recursive algorithm.
fn drop_node(nodes: &mut SlotMap<DefaultKey, Rc<Node>>, key: DefaultKey) {
    let node = nodes[key].clone();
    if let Some(parent) = node.parent {
        let parent = nodes.get_mut(parent).unwrap();
        parent.children.borrow_mut().retain(|&x| x != key);
    }

    let children = node.children.borrow().clone();
    for key in children {
        drop_node(nodes, key)
    }

    nodes.remove(key);
}

/// Composable error.
///
/// This can be handled by a parent composable with [`Catch`].
#[derive(Data, thiserror::Error)]
#[actuate(path = "crate")]
pub struct Error {
    make_error: Box<dyn Fn() -> Box<dyn core::error::Error>>,
}

impl Error {
    /// Create a new composable error.
    pub fn new(error: impl core::error::Error + Clone + 'static) -> Self {
        Self {
            make_error: Box::new(move || Box::new(error.clone())),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self.make_error)().fmt(f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self.make_error)().fmt(f)
    }
}

impl<C: Compose> Compose for Result<C, Error> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let catch_cx = use_context::<CatchContext>(&cx).unwrap();

        let child_key = use_ref(&cx, || Cell::new(None));

        let rt = Runtime::current();

        match &*cx.me() {
            Ok(content) => {
                if let Some(key) = child_key.get() {
                    let mut nodes = rt.nodes.borrow_mut();
                    let last = nodes.get_mut(key).unwrap();

                    let ptr = content as *const dyn AnyCompose;
                    let ptr: *const dyn AnyCompose = unsafe { mem::transmute(ptr) };

                    *last.compose.borrow_mut() = ComposePtr::Ptr(ptr);

                    drop(nodes);

                    rt.queue(key);
                } else {
                    let mut nodes = rt.nodes.borrow_mut();
                    let ptr: *const dyn AnyCompose =
                        unsafe { mem::transmute(content as *const dyn AnyCompose) };
                    let key = nodes.insert(Rc::new(Node {
                        compose: RefCell::new(crate::composer::ComposePtr::Ptr(ptr)),
                        scope: ScopeData::default(),
                        parent: Some(rt.current_key.get()),
                        children: RefCell::new(Vec::new()),
                        child_idx: 0,
                    }));
                    child_key.set(Some(key));

                    nodes
                        .get(rt.current_key.get())
                        .unwrap()
                        .children
                        .borrow_mut()
                        .push(key);

                    let child_state = &nodes[key].scope;

                    *child_state.contexts.borrow_mut() = cx.contexts.borrow().clone();
                    child_state
                        .contexts
                        .borrow_mut()
                        .values
                        .extend(cx.child_contexts.borrow().values.clone());

                    drop(nodes);

                    rt.queue(key);
                }
            }
            Err(error) => {
                let mut nodes = rt.nodes.borrow_mut();

                if let Some(key) = child_key.get() {
                    drop_node(&mut nodes, key);
                }

                (catch_cx.f)((error.make_error)())
            }
        }
    }
}

pub(crate) struct CatchContext {
    f: Rc<dyn Fn(Box<dyn core::error::Error>)>,
}

impl CatchContext {
    pub(crate) fn new(f: impl Fn(Box<dyn core::error::Error>) + 'static) -> Self {
        Self { f: Rc::new(f) }
    }
}

macro_rules! impl_tuples {
    ($($t:tt : $idx:tt),*) => {
        unsafe impl<$($t: Data),*> Data for ($($t,)*) {}

        impl<$($t: Compose),*> Compose for ($($t,)*) {
            fn compose(cx: Scope<Self>) -> impl Compose {
                $({
                    let ptr: *const dyn AnyCompose = unsafe { mem::transmute(&cx.me().$idx as *const dyn AnyCompose) };
                    let (key, _) = use_node(&cx, ComposePtr::Ptr(ptr), $idx);

                    let rt = Runtime::current();
                    rt.queue(key)
                })*
            }

            fn name() -> Option<Cow<'static, str>> {
                None
            }
        }
    };
}

impl_tuples!(T1:0);
impl_tuples!(T1:0, T2:1);
impl_tuples!(T1:0, T2:1, T3:2);
impl_tuples!(T1:0, T2:1, T3:2, T4:3);
impl_tuples!(T1:0, T2:1, T3:2, T4:3, T5:4);
impl_tuples!(T1:0, T2:1, T3:2, T4:3, T5:4, T6:5);
impl_tuples!(T1:0, T2:1, T3:2, T4:3, T5:4, T6:5, T7:6);
impl_tuples!(T1:0, T2:1, T3:2, T4:3, T5:4, T6:5, T7:6, T8:7);

impl<C> Compose for Vec<C>
where
    C: Compose,
{
    fn compose(cx: Scope<Self>) -> impl Compose {
        for (idx, item) in cx.me().iter().enumerate() {
            let ptr: *const dyn AnyCompose =
                unsafe { mem::transmute(item as *const dyn AnyCompose) };
            let (key, _) = use_node(&cx, ComposePtr::Ptr(ptr), idx);

            let rt = Runtime::current();
            rt.queue(key);
        }
    }
}

fn use_node(cx: ScopeState, compose_ptr: ComposePtr, child_idx: usize) -> (DefaultKey, &Rc<Node>) {
    let mut compose_ptr_cell = Some(compose_ptr);

    let (key, node) = use_ref(cx, || {
        let rt = Runtime::current();
        let mut nodes = rt.nodes.borrow_mut();

        let key = nodes.insert(Rc::new(Node {
            compose: RefCell::new(compose_ptr_cell.take().unwrap()),
            scope: ScopeData::default(),
            parent: Some(rt.current_key.get()),
            children: RefCell::new(Vec::new()),
            child_idx,
        }));

        nodes
            .get(rt.current_key.get())
            .unwrap()
            .children
            .borrow_mut()
            .push(key);

        let child_state = &nodes[key].scope;
        *child_state.contexts.borrow_mut() = cx.contexts.borrow().clone();
        child_state
            .contexts
            .borrow_mut()
            .values
            .extend(cx.child_contexts.borrow().values.clone());

        (key, nodes[key].clone())
    });

    // Reborrow the pointer to the node's composable.
    if let Some(compose_ptr) = compose_ptr_cell.take() {
        *node.compose.borrow_mut() = compose_ptr;
    }

    (*key, node)
}

pub(crate) trait AnyCompose {
    fn data_id(&self) -> TypeId;

    fn as_ptr_mut(&mut self) -> *mut ();

    unsafe fn reborrow(&mut self, ptr: *mut ());

    /// Safety: The caller must ensure `&self` is valid for the lifetime of `state`.
    unsafe fn any_compose(&self, state: &ScopeData);

    fn name(&self) -> Option<Cow<'static, str>>;
}

impl<C> AnyCompose for C
where
    C: Compose + Data,
{
    fn data_id(&self) -> TypeId {
        typeid::of::<C>()
    }

    fn as_ptr_mut(&mut self) -> *mut () {
        self as *mut Self as *mut ()
    }

    unsafe fn reborrow(&mut self, ptr: *mut ()) {
        core::ptr::swap(self, ptr as _);
    }

    unsafe fn any_compose(&self, state: &ScopeData) {
        // Reset the hook index.
        state.hook_idx.set(0);

        // Increment the scope's current generation.
        state.generation.set(state.generation.get() + 1);

        // Transmute the lifetime of `&Self`, `&ScopeData`, and the `Scope` containing both to the same`'a`.
        // Safety: `self` and `state` are guranteed to have the same lifetime..
        let state: ScopeState = unsafe { mem::transmute(state) };
        let cx: Scope<'_, C> = Scope { me: self, state };
        let cx: Scope<'_, C> = unsafe { mem::transmute(cx) };

        // Cell for the Box used to re-allocate this composable.
        let cell: &UnsafeCell<Option<Box<dyn AnyCompose>>> = use_ref(&cx, || UnsafeCell::new(None));
        // Safety: This cell is only accessed by this composable.
        let cell = unsafe { &mut *cell.get() };

        let child_key_cell = use_ref(&cx, || Cell::new(None));

        let rt = Runtime::current();

        if cell.is_none() {
            #[cfg(feature = "tracing")]
            if let Some(name) = C::name() {
                tracing::trace!("Compose: {}", name);
            }

            let child = C::compose(cx);

            if child.data_id() == typeid::of::<()>() {
                return;
            }

            let child: Box<dyn AnyCompose> = Box::new(child);
            let mut child: Box<dyn AnyCompose> = unsafe { mem::transmute(child) };

            let mut nodes = rt.nodes.borrow_mut();

            unsafe {
                if let Some(key) = child_key_cell.get() {
                    let last = nodes.get_mut(key).unwrap();
                    child.reborrow(last.compose.borrow_mut().as_ptr_mut());
                } else {
                    let child_key = nodes.insert(Rc::new(Node {
                        compose: RefCell::new(crate::composer::ComposePtr::Boxed(child)),
                        scope: ScopeData::default(),
                        parent: Some(rt.current_key.get()),
                        children: RefCell::new(Vec::new()),
                        child_idx: 0,
                    }));
                    child_key_cell.set(Some(child_key));

                    nodes
                        .get(rt.current_key.get())
                        .unwrap()
                        .children
                        .borrow_mut()
                        .push(child_key);

                    let child_state = &nodes[child_key].scope;

                    *child_state.contexts.borrow_mut() = cx.contexts.borrow().clone();
                    child_state
                        .contexts
                        .borrow_mut()
                        .values
                        .extend(cx.child_contexts.borrow().values.clone());
                }
            }
        }

        if let Some(key) = child_key_cell.get() {
            rt.queue(key)
        }
    }

    fn name(&self) -> Option<Cow<'static, str>> {
        C::name()
    }
}
