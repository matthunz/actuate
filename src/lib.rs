//! # Actuate
//! Actuate is a native, declarative, and friendly user-interface (UI) framework.
//!
//! ```no_run
//! use actuate::prelude::*;
//!
//! #[derive(Data)]
//! struct Counter {
//!     start: i32,
//! }
//!
//! impl Compose for Counter {
//!     fn compose(cx: Scope<Self>) -> impl Compose {
//!         let count = use_mut(&cx, || cx.me().start);
//!
//!         Window::new((
//!             Text::new(format!("High five count: {}", *count))
//!                 .font(GenericFamily::Cursive)
//!                 .font_size(60.),
//!             Text::new("Up high")
//!                 .on_click(move || count.update(|x| *x += 1))
//!                 .background_color(Color::BLUE),
//!             Text::new("Down low")
//!                 .on_click(move || count.update(|x| *x -= 1))
//!                 .background_color(Color::RED),
//!             if *count == 0 {
//!                 Some(Text::new("Gimme five!"))
//!             } else {
//!                 None
//!             },
//!         ))
//!         .font_size(40.)
//!     }
//! }
//!
//! actuate::run(Counter { start: 0 })
//! ```
//!
//! ## Hooks
//! Functions that begin with `use_` are called `hooks` in Actuate.
//! Hooks are used to manage state and side effects in composables.
//!
//! Hooks must be used in the same order for every re-compose.
//! Don’t use hooks inside loops, conditions, nested functions, or match blocks.
//! Instead, always use hooks at the top level of your composable, before any early returns.
//!
//! ## Features
//! - `event-loop`: Enables the `event_loop` module for access to the system event loop.
//! - `rt`: Enables the `rt` module for running async tasks on the Tokio runtime.
//! - `tracing`: Enables the `tracing` module for logging.
//! - `ui`: Enables the `ui` module for building user interfaces.

#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use slotmap::{DefaultKey, SlotMap};
use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell, UnsafeCell},
    collections::HashMap,
    fmt,
    future::Future,
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
    ops::Deref,
    pin::Pin,
    ptr::NonNull,
    rc::Rc,
    sync::{mpsc, Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
};
use thiserror::Error;
use tokio::sync::RwLock;

pub use actuate_macros::Data;

macro_rules! cfg_ui {
    ($($t:item)*) => {
        $(
            #[cfg(feature = "ui")]
            #[cfg_attr(docsrs, doc(cfg(feature = "ui")))]
            $t
        )*
    };
}

/// Prelude of commonly used items.
pub mod prelude {
    pub use crate::{
        use_context, use_drop, use_local_task, use_memo, use_mut, use_provider, use_ref, use_task,
        Cow, Data, DataField, FieldWrap, FnField, Map, Mut, Ref, RefMap, Scope, ScopeState,
        StateField, StaticField,
    };

    pub use crate::compose::{self, Compose, DynCompose, Memo};

    cfg_ui!(
        pub use crate::ui::{
            view::{use_font, Canvas, Flex, Text, View, Window},
            Draw,
        };

        pub use parley::GenericFamily;

        pub use taffy::prelude::*;

        pub use vello::peniko::Color;
    );

    #[cfg(feature = "event-loop")]
    #[cfg_attr(docsrs, doc(cfg(feature = "event-loop")))]
    pub use winit::window::WindowAttributes;
}

/// Composable functions.
pub mod compose;
use self::compose::{AnyCompose, Compose};

mod data;
pub use self::data::{Data, DataField, FieldWrap, FnField, StateField, StaticField};

#[cfg(feature = "event-loop")]
#[cfg_attr(docsrs, doc(cfg(feature = "event-loop")))]
/// System event loop for windowing.
pub mod event_loop;

#[cfg(all(feature = "rt", feature = "ui"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "rt", feature = "ui"))))]
/// Run this content on the system event loop.
pub fn run(content: impl Compose + 'static) {
    event_loop::run(ui::RenderRoot { content });
}

cfg_ui!(
    /// User interface components.
    pub mod ui;

    /// Run this content on the system event loop with a provided task executor.
    pub fn run_with_executor(content: impl Compose + 'static, executor: impl Executor + 'static) {
        event_loop::run_with_executor(ui::RenderRoot { content }, executor);
    }
);

/// Clone-on-write value.
///
/// This represents either a borrowed or owned value.
/// A borrowed value is stored as a [`RefMap`], which can be either a reference or a mapped reference.
pub enum Cow<'a, T> {
    /// Borrowed value, contained inside either a [`Ref`] or [`Map`].
    Borrowed(RefMap<'a, T>),
    /// Owned value.
    Owned(T),
}

impl<T> Cow<'_, T> {
    /// Convert or clone this value to an owned value.
    pub fn into_owned(self) -> T
    where
        T: Clone,
    {
        match self {
            Cow::Borrowed(value) => (*value).clone(),
            Cow::Owned(value) => value,
        }
    }
}

impl<T> Deref for Cow<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Cow::Borrowed(ref_map) => ref_map,
            Cow::Owned(value) => value,
        }
    }
}

impl<'a, T> From<RefMap<'a, T>> for Cow<'a, T> {
    fn from(value: RefMap<'a, T>) -> Self {
        Cow::Borrowed(value)
    }
}

impl<'a, T> From<Ref<'a, T>> for Cow<'a, T> {
    fn from(value: Ref<'a, T>) -> Self {
        RefMap::from(value).into()
    }
}

impl<'a, T> From<Map<'a, T>> for Cow<'a, T> {
    fn from(value: Map<'a, T>) -> Self {
        RefMap::from(value).into()
    }
}

/// Immutable reference or mapped reference to a value.
pub enum RefMap<'a, T> {
    /// Reference to a value.
    Ref(Ref<'a, T>),
    /// Mapped reference to a value.
    Map(Map<'a, T>),
}

impl<T> Clone for RefMap<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for RefMap<'_, T> {}

impl<T> Deref for RefMap<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            RefMap::Ref(r) => r,
            RefMap::Map(map) => map,
        }
    }
}

impl<T: Hash> Hash for RefMap<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl<'a, T> From<Ref<'a, T>> for RefMap<'a, T> {
    fn from(value: Ref<'a, T>) -> Self {
        RefMap::Ref(value)
    }
}

impl<'a, T> From<Map<'a, T>> for RefMap<'a, T> {
    fn from(value: Map<'a, T>) -> Self {
        RefMap::Map(value)
    }
}

unsafe impl<T: Data> Data for RefMap<'_, T> {
    type Id = RefMap<'static, T::Id>;
}

impl<C: Compose> Compose for RefMap<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        cx.is_container.set(true);

        let state = use_ref(&cx, || {
            let mut state = ScopeData::default();
            state.contexts = cx.contexts.clone();
            state
        });

        state.is_parent_changed.set(cx.is_parent_changed.get());

        unsafe { (**cx.me()).any_compose(state) }
    }
}

/// Mapped immutable reference to a value of type `T`.
///
/// This can be created with [`Ref::map`].
pub struct Map<'a, T> {
    ptr: *const (),
    map_fn: *const (),
    deref_fn: fn(*const (), *const ()) -> &'a T,
    generation: *const Cell<u64>,
}

impl<T> Deref for Map<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        (self.deref_fn)(self.ptr, self.map_fn)
    }
}

// Safety: The `Map` is dereferenced every re-compose, so it's guranteed not to point to
// an invalid memory location (e.g. an `Option` that previously returned `Some` is now `None`).
impl<C: Compose> Compose for Map<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        cx.is_container.set(true);

        let state = use_ref(&cx, || {
            let mut state = ScopeData::default();
            state.contexts = cx.contexts.clone();
            state
        });

        state.is_parent_changed.set(cx.is_parent_changed.get());

        unsafe { (**cx.me()).any_compose(state) }
    }

    #[cfg(feature = "tracing")]
    fn name() -> std::borrow::Cow<'static, str> {
        C::name()
    }
}

impl<T> Hash for Map<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ptr.hash(state);
        self.generation.hash(state);
    }
}

/// Immutable reference to a value of type `T`.
///
/// Memoizing this value will use pointer-equality for higher-performance.
///
/// This reference can be mapped to inner values with [`Ref::map`].
pub struct Ref<'a, T> {
    value: &'a T,
    generation: *const Cell<u64>,
}

impl<'a, T> Ref<'a, T> {
    /// Map this reference to a value of type `U`.
    pub fn map<U>(me: Self, f: fn(&T) -> &U) -> Map<'a, U> {
        Map {
            ptr: me.value as *const _ as _,
            map_fn: f as _,
            deref_fn: |ptr, g| {
                // Safety: `f` is guranteed to be a valid function pointer.
                unsafe {
                    let g: fn(&T) -> &U = mem::transmute(g);
                    g(&*(ptr as *const T))
                }
            },
            generation: me.generation,
        }
    }
}

impl<T> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T> Hash for Ref<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.value as *const T).hash(state);
        self.generation.hash(state);
    }
}

/// Mutable reference to a value of type `T`.
pub struct Mut<'a, T> {
    /// Pointer to the boxed value.
    ptr: NonNull<T>,

    /// Pointer to the scope's `is_changed` flag.
    scope_is_changed: *const Cell<bool>,

    /// Pointer to this value's generation.
    generation: *const Cell<u64>,

    /// Marker for the lifetime of this immutable reference.
    phantom: PhantomData<&'a ()>,
}

impl<'a, T: 'static> Mut<'a, T> {
    /// Queue an update to this value, triggering an update to the component owning this value.
    pub fn update(self, f: impl FnOnce(&mut T) + 'static) {
        let mut ptr = self.ptr;
        let is_changed = self.scope_is_changed;
        let generation = self.generation;

        Runtime::current().update(move || {
            let value = unsafe { ptr.as_mut() };
            f(value);

            unsafe {
                (*is_changed).set(true);

                let g = &*generation;
                g.set(g.get() + 1)
            }
        });
    }

    /// Queue an update to this value wtihout triggering an update.
    pub fn with(self, f: impl FnOnce(&mut T) + 'static) {
        let mut cell = Some(f);
        let mut ptr = self.ptr;

        Runtime::current().update(move || {
            let value = unsafe { ptr.as_mut() };
            cell.take().unwrap()(value);
        });
    }

    /// Convert this mutable reference to an immutable reference.
    pub fn as_ref(self) -> Ref<'a, T> {
        Ref {
            value: unsafe { self.ptr.as_ref() },
            generation: self.generation,
        }
    }
}

impl<T> Deref for Mut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

macro_rules! impl_pointer {
    ($($t:ident),*) => {
        $(
            impl<T> Clone for $t<'_, T> {
                fn clone(&self) -> Self {
                    *self
                }
            }

            impl<T> Copy for $t<'_, T> {}

            unsafe impl<T: Send + Sync> Send for $t<'_, T> {}

            unsafe impl<T: Sync + Sync> Sync for $t<'_, T> {}

            impl<'a, T: 'a> IntoIterator for $t<'a, T>
            where
                &'a T: IntoIterator,
            {
                type Item = <&'a T as IntoIterator>::Item;

                type IntoIter = <&'a T as IntoIterator>::IntoIter;

                fn into_iter(self) -> Self::IntoIter {
                    let value: &T = &self;
                    // Safety: the reference to `value` is guranteed to live as long as `self`.
                    let value: &T = unsafe { mem::transmute(value) };
                    value.into_iter()
                }
            }

            unsafe impl<T: Data> Data for $t<'_, T> {
                type Id = $t<'static, T::Id>;
            }
        )*
    };
}
impl_pointer!(Ref, Map, Mut);

/// An update to apply to a composable.
pub struct Update {
    f: Box<dyn FnOnce()>,
}

impl Update {
    /// Apply this update.
    ///
    /// # Safety
    /// The caller must ensure the composable triggering this update still exists.
    pub unsafe fn apply(self) {
        (self.f)();
    }
}

type RuntimeFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Runtime for a [`Composer`].
#[derive(Clone)]
pub struct Runtime {
    /// Updater for this runtime.
    updater: Arc<dyn Updater>,

    /// Local task stored on this runtime.
    tasks: Rc<RefCell<SlotMap<DefaultKey, RuntimeFuture>>>,

    /// Waker for local tasks.
    task_tx: mpsc::Sender<DefaultKey>,

    /// Update lock for shared tasks.
    lock: Arc<RwLock<()>>,
}

impl Runtime {
    /// Get the current [`Runtime`].
    ///
    /// # Panics
    /// Panics if called outside of a runtime.
    pub fn current() -> Self {
        RUNTIME.with(|runtime| {
            runtime
                .borrow()
                .as_ref()
                .expect("Runtime::current() called outside of a runtime")
                .clone()
        })
    }

    /// Enter this runtime, making it available to [`Runtime::current`].
    pub fn enter(&self) {
        RUNTIME.with(|runtime| {
            *runtime.borrow_mut() = Some(self.clone());
        });
    }

    /// Queue an update to run after [`Composer::compose`].
    pub fn update(&self, f: impl FnOnce() + 'static) {
        self.updater.update(Update { f: Box::new(f) });
    }
}

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

/// Map of [`TypeId`] to context values.
#[derive(Clone, Default)]
struct Contexts {
    values: HashMap<TypeId, Rc<dyn Any>>,
}

/// Scope state of a composable function.
pub type ScopeState<'a> = &'a ScopeData<'a>;

/// State of a composable.
#[derive(Default)]
pub struct ScopeData<'a> {
    /// Hook values stored in this scope.
    hooks: UnsafeCell<Vec<Box<dyn Any>>>,

    /// Current hook index.
    hook_idx: Cell<usize>,

    /// `true` if this scope is changed.
    is_changed: Cell<bool>,

    /// `true` if an ancestor to this scope is changed.
    is_parent_changed: Cell<bool>,

    /// `true` if this scope contains an empty composable.
    is_empty: Cell<bool>,

    /// `true` if this scope contains a container composable.
    is_container: Cell<bool>,

    /// Context values stored in this scope.
    contexts: RefCell<Contexts>,

    /// Context values for child composables.
    child_contexts: RefCell<Contexts>,

    /// Drop functions to run just before this scope is dropped.
    drops: RefCell<Vec<usize>>,

    /// Current generation of this scope.
    generation: Cell<u64>,

    /// Marker for the invariant lifetime of this scope.
    _marker: PhantomData<&'a fn(ScopeData<'a>) -> ScopeData<'a>>,
}

impl ScopeData<'_> {
    /// Set this scope as changed.
    pub fn set_changed(&self) {
        self.is_changed.set(true);
    }

    /// Returns `true` if an ancestor to this scope is changed.
    pub fn is_parent_changed(&self) -> bool {
        self.is_parent_changed.get()
    }
}

impl Drop for ScopeData<'_> {
    fn drop(&mut self) {
        for idx in &*self.drops.borrow() {
            let hooks = unsafe { &mut *self.hooks.get() };
            let any = hooks.get_mut(*idx).unwrap();
            (**any).downcast_mut::<Box<dyn FnMut()>>().unwrap()();
        }
    }
}

/// Composable scope.
pub struct Scope<'a, C> {
    me: &'a C,
    state: ScopeState<'a>,
}

impl<'a, C> Scope<'a, C> {
    /// Get a [`Ref`] to this composable.
    pub fn me(self) -> Ref<'a, C> {
        Ref {
            value: self.me,
            generation: &self.state.generation,
        }
    }

    /// Get the state of this composable.
    pub fn state(self) -> ScopeState<'a> {
        self.state
    }
}

impl<C> Clone for Scope<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for Scope<'_, C> {}

impl<'a, C> Deref for Scope<'a, C> {
    type Target = ScopeState<'a>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

/// Use an immutable reference to a value of type `T`.
///
/// `make_value` will only be called once to initialize this value.
pub fn use_ref<T: 'static>(cx: ScopeState, make_value: impl FnOnce() -> T) -> &T {
    let hooks = unsafe { &mut *cx.hooks.get() };

    let idx = cx.hook_idx.get();
    cx.hook_idx.set(idx + 1);

    let any = if idx >= hooks.len() {
        hooks.push(Box::new(make_value()));
        hooks.last().unwrap()
    } else {
        hooks.get(idx).unwrap()
    };
    (**any).downcast_ref().unwrap()
}

struct MutState<T> {
    value: T,
    generation: Cell<u64>,
}

/// Use a mutable reference to a value of type `T`.
///
/// `make_value` will only be called once to initialize this value.
pub fn use_mut<T: 'static>(cx: ScopeState, make_value: impl FnOnce() -> T) -> Mut<'_, T> {
    let hooks = unsafe { &mut *cx.hooks.get() };

    let idx = cx.hook_idx.get();
    cx.hook_idx.set(idx + 1);

    let any = if idx >= hooks.len() {
        let state = MutState {
            value: make_value(),
            generation: Cell::new(0),
        };
        hooks.push(Box::new(state));
        hooks.last_mut().unwrap()
    } else {
        hooks.get_mut(idx).unwrap()
    };
    let state: &mut MutState<T> = any.downcast_mut().unwrap();

    Mut {
        ptr: unsafe { NonNull::new_unchecked(&mut state.value as *mut _) },
        scope_is_changed: &cx.is_changed,
        generation: &state.generation,
        phantom: PhantomData::<&()>,
    }
}

/// Use a callback function.
/// The returned function will be updated to `f` whenever this component is re-composed.
pub fn use_callback<'a, T, R>(
    cx: ScopeState<'a>,
    f: impl FnMut(T) -> R + 'a,
) -> &'a Rc<dyn Fn(T) -> R + 'a>
where
    T: 'static,
    R: 'static,
{
    let f_cell: Option<Box<dyn FnMut(T) -> R + 'a>> = Some(Box::new(f));
    let mut f_cell: Option<Box<dyn FnMut(T) -> R>> = unsafe { mem::transmute(f_cell) };

    let callback = use_ref(cx, || Rc::new(RefCell::new(f_cell.take().unwrap()))).clone();

    if let Some(f) = f_cell {
        *callback.borrow_mut() = f;
    }

    use_ref(cx, move || {
        let f = callback.clone();
        Rc::new(move |input| f.borrow_mut()(input)) as Rc<dyn Fn(T) -> R>
    })
}

#[derive(Error)]
/// Error for a missing context.
pub struct ContextError<T> {
    _marker: PhantomData<T>,
}

impl<T> fmt::Debug for ContextError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContextError")
            .field(&std::any::type_name::<T>())
            .finish()
    }
}

impl<T> fmt::Display for ContextError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&format!(
            "Context value not found for type: {}",
            std::any::type_name::<T>()
        ))
    }
}

/// Use a context value of type `T`.
///
/// # Panics
/// Panics if the context value is not found.
pub fn use_context<'a, T: 'static>(cx: ScopeState<'a>) -> Result<&'a T, ContextError<T>> {
    let Some(any) = cx.contexts.borrow().values.get(&TypeId::of::<T>()).cloned() else {
        return Err(ContextError {
            _marker: PhantomData,
        });
    };

    let value: &T = (*any).downcast_ref().unwrap();
    let value: &'a T = unsafe { mem::transmute(value) };

    Ok(value)
}

/// Provide a context value of type `T`.
///
/// This value will be available to [`use_context`] to all children of this composable.
pub fn use_provider<T: 'static>(cx: ScopeState<'_>, make_value: impl FnOnce() -> T) -> Rc<T> {
    // TODO
    let r = use_ref(cx, || {
        let value = Rc::new(make_value());
        cx.child_contexts
            .borrow_mut()
            .values
            .insert(TypeId::of::<T>(), value.clone());
        value
    });
    (*r).clone()
}

/// Memoize a value, caching it until the dependency changes.
///
/// This is implemented for `T: PartialEq + 'static` by default.
/// As well as:
/// - [`Ref`]
/// - [`Map`]
/// - [`RefMap`]
/// - [`Mut`]
pub trait Memoize {
    /// Inner value to store and compare.
    type Value: PartialEq + 'static;

    /// Return the inner value for memoization.
    fn memoized(self) -> Self::Value;
}

impl<T: PartialEq + 'static> Memoize for T {
    type Value = T;

    fn memoized(self) -> Self::Value {
        self
    }
}

impl<T> Memoize for Ref<'_, T> {
    type Value = u64;

    fn memoized(self) -> Self::Value {
        unsafe { &*self.generation }.get()
    }
}

impl<T> Memoize for Map<'_, T> {
    type Value = u64;

    fn memoized(self) -> Self::Value {
        unsafe { &*self.generation }.get()
    }
}

impl<T> Memoize for RefMap<'_, T> {
    type Value = u64;

    fn memoized(self) -> Self::Value {
        match self {
            RefMap::Ref(r) => r.memoized(),
            RefMap::Map(map) => map.memoized(),
        }
    }
}

impl<T> Memoize for Mut<'_, T> {
    type Value = u64;

    fn memoized(self) -> Self::Value {
        unsafe { &*self.generation }.get()
    }
}

/// Use a memoized value of type `T` with a dependency of type `D`.
///
/// `make_value` will update the returned value whenver `dependency` is changed.
pub fn use_memo<D, T>(cx: ScopeState, dependency: D, make_value: impl FnOnce() -> T) -> Ref<T>
where
    D: Memoize,
    T: 'static,
{
    let mut dependency_cell = Some(dependency.memoized());

    let mut make_value_cell = Some(make_value);
    let value_mut = use_mut(cx, || make_value_cell.take().unwrap()());

    let hash_mut = use_mut(cx, || dependency_cell.take().unwrap());

    if let Some(make_value) = make_value_cell {
        if let Some(dependency) = dependency_cell.take() {
            if dependency != *hash_mut {
                let value = make_value();
                value_mut.with(move |update| *update = value);

                hash_mut.with(move |dst| *dst = dependency);
            }
        }
    }

    value_mut.as_ref()
}

/// Use a function that will be called when this scope is dropped.
pub fn use_drop<'a>(cx: ScopeState<'a>, f: impl FnOnce() + 'a) {
    let mut f_cell = Some(f);

    let cell = use_ref(cx, || {
        let f: Box<dyn FnOnce()> = Box::new(f_cell.take().unwrap());

        // Safety `f` is guranteed to live as long as `cx`.
        let f: Box<dyn FnOnce()> = unsafe { mem::transmute(f) };

        RefCell::new(Some(f))
    });

    let idx = cx.hook_idx.get();
    use_ref(cx, || {
        cx.drops.borrow_mut().push(idx);

        let f: Box<dyn FnMut()> = Box::new(move || {
            cell.borrow_mut().take().unwrap()();
        });

        // Safety `f` is guranteed to live as long as `cx`.
        let f: Box<dyn FnMut()> = unsafe { mem::transmute(f) };
        f
    });

    if let Some(f) = f_cell {
        let f: Box<dyn FnOnce()> = Box::new(f);

        // Safety `f` is guranteed to live as long as `cx`.
        let f: Box<dyn FnOnce()> = unsafe { mem::transmute(f) };

        *cell.borrow_mut() = Some(f);
    }
}

struct TaskWaker {
    key: DefaultKey,
    updater: Arc<dyn Updater>,
    tx: mpsc::Sender<DefaultKey>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        let key = self.key;
        let pending = self.tx.clone();
        self.updater.update(Update {
            f: Box::new(move || {
                pending.send(key).unwrap();
            }),
        });
    }
}

/// Use a local task that runs on the current thread.
///
/// This will run on the window event loop, polling the task until it completes.
pub fn use_local_task<'a, F>(cx: ScopeState<'a>, make_task: impl FnOnce() -> F)
where
    F: Future<Output = ()> + 'a,
{
    let key = *use_ref(cx, || {
        let task: Pin<Box<dyn Future<Output = ()>>> = Box::pin(make_task());
        let task: Pin<Box<dyn Future<Output = ()>>> = unsafe { mem::transmute(task) };

        let rt = Runtime::current();
        let key = rt.tasks.borrow_mut().insert(task);
        rt.task_tx.send(key).unwrap();
        key
    });

    use_drop(cx, move || {
        Runtime::current().tasks.borrow_mut().remove(key);
    })
}

struct WrappedFuture {
    lock: Arc<Mutex<bool>>,
    task: Pin<Box<dyn Future<Output = ()> + Send>>,
    rt: Runtime,
}

impl Future for WrappedFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        let guard = me.lock.lock().unwrap();

        if *guard {
            me.rt.enter();

            let _guard = Box::pin(me.rt.lock.read()).as_mut().poll(cx);

            me.task.as_mut().poll(cx)
        } else {
            Poll::Ready(())
        }
    }
}

unsafe impl Send for WrappedFuture {}

/// Executor for async tasks.
pub trait Executor {
    /// Spawn a future on this executor.
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static;
}

impl<T: Executor> Executor for Box<T> {
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        (**self).spawn(future);
    }
}

#[cfg(feature = "rt")]
#[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
impl Executor for tokio::runtime::Runtime {
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn(future);
    }
}

trait AnyExecutor {
    fn spawn_any(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>);
}

impl<E: Executor> AnyExecutor for E {
    fn spawn_any(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.spawn(future);
    }
}

/// Context for the Tokio runtime.
pub struct RuntimeContext {
    rt: Box<dyn AnyExecutor>,
}

impl RuntimeContext {
    /// Spawn a future on the current runtime.
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.rt.spawn_any(Box::pin(future));
    }
}

/// Use a multi-threaded task that runs on a separate thread.
///
/// This will run on the [Tokio](https://docs.rs/tokio/latest/tokio/) runtime, polling the task until it completes.
pub fn use_task<'a, F>(cx: ScopeState<'a>, make_task: impl FnOnce() -> F)
where
    F: Future<Output = ()> + Send + 'a,
{
    let runtime_cx = use_context::<RuntimeContext>(cx).unwrap();
    let lock = use_ref(cx, || {
        let lock = Arc::new(Mutex::new(true));

        // Safety: `task`` is guaranteed to live as long as `cx`, and is disabled after the scope is dropped.
        let task: Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(make_task());
        let task: Pin<Box<dyn Future<Output = ()> + Send>> = unsafe { mem::transmute(task) };

        runtime_cx.rt.spawn_any(Box::pin(WrappedFuture {
            lock: lock.clone(),
            task,
            rt: Runtime::current(),
        }));

        lock
    });

    // Disable this task after the scope is dropped.
    use_drop(cx, || {
        *lock.lock().unwrap() = false;
    });
}

/// Updater for a [`Composer`].
pub trait Updater: Send + Sync {
    /// Update the content of a [`Composer`].
    fn update(&self, update: Update);
}

struct DefaultUpdater;

impl Updater for DefaultUpdater {
    fn update(&self, update: crate::Update) {
        unsafe {
            update.apply();
        }
    }
}

struct UpdateWrapper<U> {
    updater: U,
    lock: Arc<RwLock<()>>,
}

impl<U: Updater> Updater for UpdateWrapper<U> {
    fn update(&self, update: crate::Update) {
        let lock = self.lock.clone();
        self.updater.update(Update {
            f: Box::new(move || {
                let _guard = lock.blocking_write();
                unsafe { update.apply() }
            }),
        });
    }
}

/// Composer for composable content.
pub struct Composer {
    compose: Box<dyn AnyCompose>,
    scope_state: Box<ScopeData<'static>>,
    rt: Runtime,
    task_rx: mpsc::Receiver<DefaultKey>,
}

impl Composer {
    /// Create a new [`Composer`] with the given content and default updater.
    #[cfg(feature = "rt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
    pub fn new(content: impl Compose + 'static) -> Self {
        let rt = tokio::runtime::Runtime::new().unwrap();
        Self::with_updater(content, DefaultUpdater, rt)
    }

    /// Create a new [`Composer`] with the given content, updater, and task executor.
    pub fn with_updater(
        content: impl Compose + 'static,
        updater: impl Updater + 'static,
        executor: impl Executor + 'static,
    ) -> Self {
        let lock = Arc::new(RwLock::new(()));
        let updater = Arc::new(UpdateWrapper {
            updater,
            lock: lock.clone(),
        });
        let (task_tx, task_rx) = mpsc::channel();

        let scope_data = ScopeData::default();
        scope_data.child_contexts.borrow_mut().values.insert(
            TypeId::of::<RuntimeContext>(),
            Rc::new(RuntimeContext {
                rt: Box::new(executor),
            }),
        );

        Self {
            compose: Box::new(content),
            scope_state: Box::new(scope_data),
            rt: Runtime {
                updater: updater.clone(),
                tasks: Rc::new(RefCell::new(SlotMap::new())),
                task_tx,
                lock,
            },
            task_rx,
        }
    }

    /// Compose the content of this composer.
    pub fn compose(&mut self) {
        self.rt.enter();

        while let Ok(key) = self.task_rx.try_recv() {
            let waker = Waker::from(Arc::new(TaskWaker {
                key,
                updater: Runtime::current().updater.clone(),
                tx: self.rt.task_tx.clone(),
            }));
            let mut cx = Context::from_waker(&waker);

            let mut tasks = self.rt.tasks.borrow_mut();
            let task = tasks.get_mut(key).unwrap();
            let _ = task.as_mut().poll(&mut cx);
        }

        // Safety: `self.compose` is guaranteed to live as long as `self.scope_state`.
        unsafe { self.compose.any_compose(&self.scope_state) }
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, Composer};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    #[derive(Data)]
    struct Counter {
        x: Rc<Cell<i32>>,
    }

    impl Compose for Counter {
        fn compose(cx: Scope<Self>) -> impl Compose {
            cx.me().x.set(cx.me().x.get() + 1);

            cx.set_changed();
        }
    }

    #[derive(Data)]
    struct NonUpdateCounter {
        x: Rc<Cell<i32>>,
    }

    impl Compose for NonUpdateCounter {
        fn compose(cx: Scope<Self>) -> impl Compose {
            cx.me().x.set(cx.me().x.get() + 1);
        }
    }

    #[test]
    fn it_composes() {
        #[derive(Data)]
        struct Wrap {
            x: Rc<Cell<i32>>,
        }

        impl Compose for Wrap {
            fn compose(cx: Scope<Self>) -> impl Compose {
                Counter {
                    x: cx.me().x.clone(),
                }
            }
        }

        let x = Rc::new(Cell::new(0));
        let mut composer = Composer::new(Wrap { x: x.clone() });

        composer.compose();
        assert_eq!(x.get(), 1);

        composer.compose();
        assert_eq!(x.get(), 2);
    }

    #[test]
    fn it_skips_recomposes() {
        #[derive(Data)]
        struct Wrap {
            x: Rc<Cell<i32>>,
        }

        impl Compose for Wrap {
            fn compose(cx: Scope<Self>) -> impl Compose {
                NonUpdateCounter {
                    x: cx.me().x.clone(),
                }
            }
        }

        let x = Rc::new(Cell::new(0));
        let mut composer = Composer::new(Wrap { x: x.clone() });

        composer.compose();
        assert_eq!(x.get(), 1);

        composer.compose();
        assert_eq!(x.get(), 1);
    }

    #[test]
    fn it_composes_any_compose() {
        #[derive(Data)]
        struct Wrap {
            x: Rc<Cell<i32>>,
        }

        impl Compose for Wrap {
            fn compose(cx: crate::Scope<Self>) -> impl Compose {
                DynCompose::new(Counter {
                    x: cx.me().x.clone(),
                })
            }
        }

        let x = Rc::new(Cell::new(0));
        let mut composer = Composer::new(Wrap { x: x.clone() });

        composer.compose();
        assert_eq!(x.get(), 1);

        composer.compose();
        assert_eq!(x.get(), 2);
    }

    #[test]
    fn it_memoizes_composables() {
        #[derive(Data)]
        struct B {
            x: Rc<RefCell<i32>>,
        }

        impl Compose for B {
            fn compose(cx: Scope<Self>) -> impl Compose {
                *cx.me().x.borrow_mut() += 1;
            }
        }

        #[derive(Data)]
        struct A {
            x: Rc<RefCell<i32>>,
        }

        impl Compose for A {
            fn compose(cx: Scope<Self>) -> impl Compose {
                let x = cx.me().x.clone();
                Memo::new((), B { x })
            }
        }

        let x = Rc::new(RefCell::new(0));
        let mut compsoer = Composer::new(A { x: x.clone() });

        compsoer.compose();
        assert_eq!(*x.borrow(), 1);

        compsoer.compose();
        assert_eq!(*x.borrow(), 1);
    }
}
