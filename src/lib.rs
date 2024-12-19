#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://avatars.githubusercontent.com/u/161107368",
    html_favicon_url = "https://avatars.githubusercontent.com/u/161107368"
)]

//! # Actuate
//! A high-performance and borrow-checker friendly framework for declarative programming in Rust.
//! This crate provides a generic library that lets you define reactive components
//! (also known as composables, for more see [`Compose`]).
//!
//! ```no_run
//! use actuate::prelude::*;
//! use bevy::prelude::*;
//!
//! // Counter composable.
//! #[derive(Data)]
//! struct Counter {
//!     start: i32,
//! }
//!
//! impl Compose for Counter {
//!     fn compose(cx: Scope<Self>) -> impl Compose {
//!         let count = use_mut(&cx, || cx.me().start);
//!
//!         material_ui((
//!             text::headline(format!("High five count: {}", count)),
//!             button(text::label("Up high")).on_click(move || SignalMut::update(count, |x| *x += 1)),
//!             button(text::label("Down low")).on_click(move || SignalMut::update(count, |x| *x -= 1)),
//!             if *count == 0 {
//!                 Some(text::label("Gimme five!"))
//!             } else {
//!                 None
//!             },
//!         ))
//!         .align_items(AlignItems::Center)
//!         .justify_content(JustifyContent::Center)
//!     }
//! }
//!```
//!
//! ## Borrowing
//! Composables can borrow from their ancestors, as well as state.
//! ```no_run
//! use actuate::prelude::*;
//! use bevy::prelude::*;
//!
//! #[derive(Data)]
//! struct User<'a> {
//!     // `actuate::Cow` allows for either a borrowed or owned value.
//!     name: Cow<'a, String>,
//! }
//!
//! impl Compose for User<'_> {
//!     fn compose(cx: Scope<Self>) -> impl Compose {
//!         text::headline(cx.me().name.to_string())
//!     }
//! }
//!
//! #[derive(Data)]
//! struct App {
//!     name: String
//! }
//!
//! impl Compose for App {
//!     fn compose(cx: Scope<Self>) -> impl Compose {
//!         // Get a mapped reference to the app's `name` field.
//!         let name = Signal::map(cx.me(), |me| &me.name).into();
//!
//!         User { name }
//!     }
//! }
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
//! ## Installation
//! To add this crate to your project:
//! ```sh
//! cargo add actuate --features full
//! ```
//!
//! ## Features
//! - `std`: Enables features that use Rust's standard library (default).
//!    With this feature disabled Actuate can be used in `#![no_std]` environments.
//! - `animation`: Enables the `animation` module for animating values from the [Bevy](https://crates.io/crates/bevy) ECS.
//!   (enables the `ecs` feature).
//! - `ecs`: Enables the `ecs` module for bindings to the [Bevy](https://crates.io/crates/bevy) ECS.
//! - `executor`: Enables the `executor` module for multi-threaded tasks.
//! - `material`: Enables the `material` module for Material UI (enables the `ecs` and `ui` features).
//! - `picking`: Enables support for picking event handlers with `Modify` (requires the `ecs` feature).
//! - `rt` Enables support for the [Tokio](https://crates.io/crates/tokio) runtime with the Executor trait.
//!   (enables the `executor` feature).
//! - `tracing`: Enables the logging through the `tracing` crate.
//! - `ui`: Enables the `ui` module for user interface components.
//! - `full`: Enables all features above.

extern crate alloc;

use ahash::AHasher;
use alloc::rc::Rc;
use core::{
    any::{Any, TypeId},
    cell::{Cell, RefCell, UnsafeCell},
    fmt,
    future::Future,
    hash::{BuildHasherDefault, Hash, Hasher},
    marker::PhantomData,
    mem,
    ops::Deref,
    pin::Pin,
    ptr::NonNull,
};
use slotmap::DefaultKey;
use thiserror::Error;

#[cfg(not(feature = "std"))]
use hashbrown::HashMap;

#[cfg(feature = "std")]
use std::collections::HashMap;

/// Prelude of commonly used items.
pub mod prelude {
    pub use crate::{
        compose::{self, catch, dyn_compose, memo, Compose, DynCompose, Error, Memo},
        data::{data, Data},
        use_callback, use_context, use_drop, use_local_task, use_memo, use_mut, use_provider,
        use_ref, Cow, Generational, Map, RefMap, Scope, ScopeState, Signal, SignalMut,
    };

    #[cfg(feature = "animation")]
    #[cfg_attr(docsrs, doc(cfg(feature = "animation")))]
    pub use crate::animation::{use_animated, UseAnimated};

    #[cfg(feature = "ecs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "ecs")))]
    pub use crate::ecs::{
        spawn, use_bundle, use_commands, use_world, use_world_once, ActuatePlugin, Composition,
        Modifier, Modify, Spawn, UseCommands,
    };

    #[cfg(feature = "executor")]
    #[cfg_attr(docsrs, doc(cfg(feature = "executor")))]
    pub use crate::use_task;

    #[cfg(feature = "ui")]
    #[cfg_attr(docsrs, doc(cfg(feature = "ui")))]
    pub use crate::ui::{scroll_view, ScrollView};

    #[cfg(feature = "material")]
    #[cfg_attr(docsrs, doc(cfg(feature = "material")))]
    pub use crate::ui::material::{
        button, container, material_ui, radio_button, switch, text, Button, MaterialUi,
        RadioButton, Switch, Theme, TypographyKind, TypographyStyleKind,
    };
}

#[cfg(feature = "animation")]
#[cfg_attr(docsrs, doc(cfg(feature = "animation")))]
/// Animation hooks.
pub mod animation;

/// Composable functions.
pub mod compose;
use self::compose::{AnyCompose, Compose};

/// Low-level composer.
pub mod composer;
use self::composer::Runtime;

/// Data trait and macros.
pub mod data;
use crate::data::Data;

#[cfg(feature = "ecs")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecs")))]
/// Bevy ECS integration.
pub mod ecs;

#[cfg(feature = "executor")]
#[cfg_attr(docsrs, doc(cfg(feature = "executor")))]
/// Task execution context.
pub mod executor;

#[cfg(feature = "ui")]
#[cfg_attr(docsrs, doc(cfg(feature = "ui")))]
/// User interface components.
pub mod ui;

/// Clone-on-write value.
///
/// This represents either a borrowed or owned value.
/// A borrowed value is stored as a [`RefMap`], which can be either a reference or a mapped reference.
#[derive(Debug)]
pub enum Cow<'a, T> {
    /// Borrowed value, contained inside either a [`Signal`] or [`Map`].
    Borrowed(RefMap<'a, T>),
    /// Owned value.
    Owned(T),
}

impl<T> Cow<'_, T> {
    /// Clone this value to an owned value.
    pub fn to_owned(&self) -> T
    where
        T: Clone,
    {
        self.clone().into_owned()
    }

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

impl<T> Clone for Cow<'_, T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Cow::Borrowed(value) => Cow::Borrowed(*value),
            Cow::Owned(value) => Cow::Owned(value.clone()),
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

impl<'a, T> From<Signal<'a, T>> for Cow<'a, T> {
    fn from(value: Signal<'a, T>) -> Self {
        RefMap::from(value).into()
    }
}

impl<'a, T> From<Map<'a, T>> for Cow<'a, T> {
    fn from(value: Map<'a, T>) -> Self {
        RefMap::from(value).into()
    }
}

impl<T: fmt::Display> fmt::Display for Cow<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cow::Borrowed(value) => value.fmt(f),
            Cow::Owned(value) => value.fmt(f),
        }
    }
}

unsafe impl<T: Data> Data for Cow<'_, T> {}

/// Immutable reference or mapped reference to a value.
#[derive(Debug)]
pub enum RefMap<'a, T> {
    /// Reference to a value.
    Ref(&'a T),
    /// Signal value.
    Signal(Signal<'a, T>),
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
            RefMap::Signal(s) => s,
            RefMap::Map(map) => map,
        }
    }
}

impl<T: Hash> Hash for RefMap<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl<'a, T> From<Signal<'a, T>> for RefMap<'a, T> {
    fn from(value: Signal<'a, T>) -> Self {
        RefMap::Signal(value)
    }
}

impl<'a, T> From<Map<'a, T>> for RefMap<'a, T> {
    fn from(value: Map<'a, T>) -> Self {
        RefMap::Map(value)
    }
}

impl<T: fmt::Display> fmt::Display for RefMap<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

unsafe impl<T: Data> Data for RefMap<'_, T> {}

/// Mapped immutable reference to a value of type `T`.
///
/// This can be created with [`Signal::map`].
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

/// Unchecked, mapped immutable reference to a value of type `T`.
///
/// This can be created with [`Signal::map_unchecked`].
pub struct MapUnchecked<'a, T> {
    map: Map<'a, T>,
}

unsafe impl<T> Data for MapUnchecked<'_, T> {}

impl<C: Compose> Compose for MapUnchecked<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        // Safety: The `Map` is dereferenced every re-compose, so it's guranteed not to point to
        // an invalid memory location (e.g. an `Option` that previously returned `Some` is now `None`).
        unsafe { (*cx.me().map).any_compose(cx.state) }
    }

    fn name() -> Option<std::borrow::Cow<'static, str>> {
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
/// This reference can be mapped to inner values with [`Signal::map`].
pub struct Signal<'a, T> {
    /// Pinned reference to the value.
    value: &'a T,

    /// Pointer to this value's current generation.
    generation: *const Cell<u64>,
}

impl<'a, T> Signal<'a, T> {
    /// Map this reference to a value of type `U`.
    pub fn map<U>(me: Self, f: fn(&T) -> &U) -> Map<'a, U> {
        Map {
            ptr: me.value as *const _ as _,
            map_fn: f as _,
            deref_fn: |ptr, g| {
                // Safety: `f` is guaranteed to be a valid function pointer.
                unsafe {
                    let g: fn(&T) -> &U = mem::transmute(g);
                    g(&*(ptr as *const T))
                }
            },
            generation: me.generation,
        }
    }

    /// Unsafely map this reference to a value of type `U`.
    /// The returned `MapUnchecked` implements `Compose` to allow for borrowed child composables.
    ///
    /// # Safety
    /// The returned `MapUnchecked` must only be returned once.
    /// Composing the same `MapUnchecked` at multiple locations in the tree at the same time will result in undefined behavior.
    pub unsafe fn map_unchecked<U>(me: Self, f: fn(&T) -> &U) -> MapUnchecked<'a, U> {
        MapUnchecked {
            map: Signal::map(me, f),
        }
    }
}

impl<T> Deref for Signal<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T> Hash for Signal<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.value as *const T).hash(state);
        self.generation.hash(state);
    }
}

#[derive(Clone, Copy)]
struct UnsafeWrap<T: ?Sized>(T);

unsafe impl<T: ?Sized> Send for UnsafeWrap<T> {}

unsafe impl<T: ?Sized> Sync for UnsafeWrap<T> {}

/// Mutable reference to a value of type `T`.
pub struct SignalMut<'a, T> {
    /// Pointer to the boxed value.
    ptr: NonNull<T>,

    /// Key to this signal's scope.
    scope_key: DefaultKey,

    /// Pointer to this value's generation.
    generation: *const Cell<u64>,

    /// Marker for the lifetime of this immutable reference.
    _marker: PhantomData<&'a ()>,
}

impl<'a, T: 'static> SignalMut<'a, T> {
    /// Queue an update to this value, triggering an update to the component owning this value.
    pub fn update(me: Self, f: impl FnOnce(&mut T) + Send + 'static) {
        let scope_key = me.scope_key;

        Self::with(me, move |value| {
            let rt = Runtime::current();
            rt.queue(scope_key);

            f(value)
        })
    }

    /// Queue an update to this value, triggering an update to the component owning this value.
    pub fn set(me: Self, value: T)
    where
        T: Send,
    {
        SignalMut::update(me, |x| *x = value)
    }

    /// Queue an update to this value if it is not equal to the given value.
    pub fn set_if_neq(me: Self, value: T)
    where
        T: PartialEq + Send,
    {
        if *me != value {
            SignalMut::set(me, value);
        }
    }

    /// Queue an update to this value without triggering an update.
    pub fn with(me: Self, f: impl FnOnce(&mut T) + Send + 'static) {
        let cell = UnsafeWrap(Some(f));
        let ptr = UnsafeWrap(me.ptr);
        let generation_ptr = UnsafeWrap(me.generation);

        Runtime::current().update(move || {
            let mut cell = cell;
            let mut ptr = ptr;
            let generation_ptr = generation_ptr;

            // Safety: Updates are guaranteed to be called before any structural changes of the composition tree.
            let value = unsafe { ptr.0.as_mut() };
            cell.0.take().unwrap()(value);

            // Increment the generation of this value.
            // Safety: the pointer to this scope's generation is guranteed to outlive `me`.
            let generation = unsafe { &*generation_ptr.0 };
            generation.set(generation.get() + 1)
        });
    }

    /// Convert this mutable reference to an immutable reference.
    pub fn as_ref(me: Self) -> Signal<'a, T> {
        Signal {
            value: unsafe { me.ptr.as_ref() },
            generation: me.generation,
        }
    }
}

impl<T> Deref for SignalMut<'_, T> {
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

            impl<T: fmt::Debug> fmt::Debug for $t<'_, T> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.debug_struct(stringify!($t))
                        .field("value", &**self)
                        .field("generation", &unsafe { &*self.generation }.get())
                        .finish()
                }
            }

            impl<T: fmt::Display> fmt::Display for $t<'_, T> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    (&**self).fmt(f)
                }
            }

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

            unsafe impl<T: Data> Data for $t<'_, T> {}
        )*
    };
}
impl_pointer!(Signal, Map, SignalMut);

/// Map of [`TypeId`] to context values.
#[derive(Clone, Default)]
struct Contexts {
    values: HashMap<TypeId, Rc<dyn Any>, BuildHasherDefault<AHasher>>,
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
pub struct Scope<'a, C: ?Sized> {
    me: &'a C,
    state: ScopeState<'a>,
}

impl<'a, C> Scope<'a, C> {
    /// Get a [`Signal`] to this composable.
    pub fn me(self) -> Signal<'a, C> {
        Signal {
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
pub fn use_mut<T: 'static>(cx: ScopeState, make_value: impl FnOnce() -> T) -> SignalMut<'_, T> {
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

    SignalMut {
        ptr: unsafe { NonNull::new_unchecked(&mut state.value as *mut _) },
        scope_key: Runtime::current().current_key.get(),
        generation: &state.generation,
        _marker: PhantomData,
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

impl<T> Clone for ContextError<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ContextError<T> {}

impl<T> fmt::Debug for ContextError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ContextError")
            .field(&core::any::type_name::<T>())
            .finish()
    }
}

impl<T> fmt::Display for ContextError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!(
            "Context value not found for type: {}",
            core::any::type_name::<T>()
        ))
    }
}

/// Use a context value of type `T`.
///
/// This context must have already been provided by a parent composable with [`use_provider`],
/// otherwise this function will return a [`ContextError`].
pub fn use_context<T: 'static>(cx: ScopeState) -> Result<&Rc<T>, ContextError<T>> {
    let result = use_ref(cx, || {
        let Some(any) = cx.contexts.borrow().values.get(&TypeId::of::<T>()).cloned() else {
            return Err(ContextError {
                _marker: PhantomData,
            });
        };

        let value: Rc<T> = Rc::downcast(any).unwrap();
        Ok(value)
    });

    result.as_ref().map_err(|e| *e)
}

/// Provide a context value of type `T`.
///
/// This value will be available to [`use_context`] to all children of this composable.
pub fn use_provider<T: 'static>(cx: ScopeState<'_>, make_value: impl FnOnce() -> T) -> &Rc<T> {
    use_ref(cx, || {
        let value = Rc::new(make_value());
        cx.child_contexts
            .borrow_mut()
            .values
            .insert(TypeId::of::<T>(), value.clone());
        value
    })
}

/// Generational reference.
/// This can be used to compare expensive values by pointer equality.
///
/// This trait is implemented for:
/// - [`Signal`]
/// - [`Map`]
/// - [`SignalMut`]
pub trait Generational {
    /// Get the current generation of this value.
    fn generation(self) -> u64;
}

impl<T> Generational for Signal<'_, T> {
    fn generation(self) -> u64 {
        // Safety: This pointer is valid for `'a`.
        unsafe { &*self.generation }.get()
    }
}

impl<T> Generational for Map<'_, T> {
    fn generation(self) -> u64 {
        // Safety: This pointer is valid for `'a`.
        unsafe { &*self.generation }.get()
    }
}

impl<T> Generational for SignalMut<'_, T> {
    fn generation(self) -> u64 {
        // Safety: This pointer is valid for `'a`.
        unsafe { &*self.generation }.get()
    }
}

/// Use an effect that will run whenever the provided dependency is changed.
pub fn use_effect<D, T>(cx: ScopeState, dependency: D, effect: impl FnOnce(&D))
where
    D: PartialEq + Send + 'static,
{
    let mut dependency_cell = Some(dependency);

    let last_mut = use_mut(cx, || dependency_cell.take().unwrap());

    if let Some(dependency) = dependency_cell.take() {
        if dependency != *last_mut {
            effect(&dependency);

            SignalMut::set(last_mut, dependency);
        }
    } else {
        effect(&last_mut);
    }
}

/// Use a memoized value of type `T` with a dependency of type `D`.
///
/// `make_value` will update the returned value whenver `dependency` is changed.
pub fn use_memo<D, T>(cx: ScopeState, dependency: D, make_value: impl FnOnce() -> T) -> Signal<T>
where
    D: PartialEq + Send + 'static,
    T: Send + 'static,
{
    let mut dependency_cell = Some(dependency);
    let mut make_value_cell = Some(make_value);

    let value_mut = use_mut(cx, || make_value_cell.take().unwrap()());
    let last_mut = use_mut(cx, || dependency_cell.take().unwrap());

    if let Some(make_value) = make_value_cell {
        if let Some(dependency) = dependency_cell.take() {
            if dependency != *last_mut {
                let value = make_value();
                SignalMut::with(value_mut, move |update| *update = value);

                SignalMut::with(last_mut, move |dst| *dst = dependency);
            }
        }
    }

    SignalMut::as_ref(value_mut)
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

/// Use a local task that runs on the current thread.
///
/// This will run on the window event loop, polling the task until it completes.
///
/// # Examples
///
/// Sending child state to parents.
///
/// ```
/// use actuate::prelude::*;
/// use tokio::sync::mpsc;
/// use std::cell::Cell;
///
/// #[derive(Data)]
/// struct Child<'a> {
///     idx: usize,
///     tx: &'a mpsc::UnboundedSender<usize>,
/// }
///
/// impl Compose for Child<'_> {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         cx.me().tx.send(cx.me().idx).unwrap();  
///     }
/// }
///
/// #[derive(Data)]
/// struct App;
///
/// impl Compose for App {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         let (tx, ref rx_cell) = use_ref(&cx, || {
///         let (tx, rx) = mpsc::unbounded_channel();
///             (tx, Cell::new(Some(rx)))
///         });
///
///         use_local_task(&cx, move || async move {
///             let mut rx = rx_cell.take().unwrap();
///             while let Some(id) = rx.recv().await {
///                 dbg!("Composed: {}", id);
///             }
///         });
///
///         (
///             Child { idx: 0, tx },
///             Child { idx: 1, tx }
///         )
///    }
/// }
/// ```
pub fn use_local_task<'a, F>(cx: ScopeState<'a>, make_task: impl FnOnce() -> F)
where
    F: Future<Output = ()> + 'a,
{
    let key = *use_ref(cx, || {
        let task: Pin<Box<dyn Future<Output = ()>>> = Box::pin(make_task());
        let task: Pin<Box<dyn Future<Output = ()>>> = unsafe { mem::transmute(task) };

        let rt = Runtime::current();
        let key = rt.tasks.borrow_mut().insert(task);
        rt.task_queue.push(key);
        key
    });

    use_drop(cx, move || {
        Runtime::current().tasks.borrow_mut().remove(key);
    })
}

#[cfg(feature = "executor")]
type BoxedFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

#[cfg(feature = "executor")]
struct TaskFuture {
    task: alloc::sync::Arc<std::sync::Mutex<Option<BoxedFuture>>>,
    rt: Runtime,
}

#[cfg(feature = "executor")]
impl Future for TaskFuture {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Self::Output> {
        let me = &mut *self;

        // Lock the guard on this task.
        // This is to ensure the scope for this task is not dropped while polling.
        let mut guard = me.task.lock().unwrap();

        if let Some(task) = &mut *guard {
            me.rt.enter();

            let _guard = Box::pin(me.rt.lock.read()).as_mut().poll(cx);

            task.as_mut().poll(cx)
        } else {
            // The scope is dropped, we must complete this task early.
            std::task::Poll::Ready(())
        }
    }
}

#[cfg(feature = "executor")]
unsafe impl Send for TaskFuture {}

#[cfg(feature = "executor")]
#[cfg_attr(docsrs, doc(cfg(feature = "executor")))]
/// Use a multi-threaded task that runs on a separate thread.
///
/// This will run on the current [`Executor`](`crate::executor::Executor`), polling the task until it completes.
///
/// # Examples
///
/// ```
/// use actuate::prelude::*;
/// use bevy::prelude::*;
/// use serde::Deserialize;
/// use std::collections::HashMap;
///
/// // Dog breed composable.
/// #[derive(Data)]
/// struct Breed {
///     name: String,
///     families: Vec<String>,
/// }
///
/// impl Compose for Breed {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         container((
///             text::headline(cx.me().name.to_owned()),
///             compose::from_iter(cx.me().families.clone(), |family| {
///                 text::label(family.to_string())
///             }),
///         ))
///     }
/// }
///
/// #[derive(Deserialize)]
/// struct Response {
///     message: HashMap<String, Vec<String>>,
/// }
///
/// // Dog breed list composable.
/// #[derive(Data)]
/// struct BreedList;
///
/// impl Compose for BreedList {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         let breeds = use_mut(&cx, HashMap::new);
///
///         // Spawn a task that loads dog breeds from an HTTP API.
///         use_task(&cx, move || async move {
///             let json: Response = reqwest::get("https://dog.ceo/api/breeds/list/all")
///                 .await
///                 .unwrap()
///                 .json()
///                 .await
///                 .unwrap();
///
///             SignalMut::set(breeds, json.message);
///         });
///
///         // Render the currently loaded breeds.
///         scroll_view(compose::from_iter((*breeds).clone(), |breed| Breed {
///             name: breed.0.clone(),
///             families: breed.1.clone(),
///         }))
///         .flex_gap(Val::Px(30.))
///     }
/// }
/// ```
pub fn use_task<'a, F>(cx: ScopeState<'a>, make_task: impl FnOnce() -> F)
where
    F: Future<Output = ()> + Send + 'a,
{
    let runtime_cx = use_context::<executor::ExecutorContext>(cx).unwrap();
    let task_lock = use_ref(cx, || {
        // Safety: `task`` is guaranteed to live as long as `cx`, and is disabled after the scope is dropped.
        let task: Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(make_task());
        let task: Pin<Box<dyn Future<Output = ()> + Send>> = unsafe { mem::transmute(task) };
        let task_lock = std::sync::Arc::new(std::sync::Mutex::new(Some(task)));

        runtime_cx.executor.spawn(Box::pin(TaskFuture {
            task: task_lock.clone(),
            rt: Runtime::current(),
        }));

        task_lock
    });

    // Disable this task after the scope is dropped.
    use_drop(cx, || {
        *task_lock.lock().unwrap() = None;
    });
}
