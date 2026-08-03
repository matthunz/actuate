//! Data trait and macros.
//!
//! # Data
//!
//! [`Data`] is a trait that enforces pinned references to compososition state.
//!
//! The `#[derive(Data)]` macro can be used to derive the [`Data`] trait for a struct.
//! This requires the struct's fields either:
//! - Implement the [`Data`] trait.
//! - Are `'static`.
//! - Are functions that take `'static` arguments and return a type that implements the [`Data`] trait.
//!
//! # Trait objects
//!
//! Trait objects can also borrow from state:
//!
//! ```no_run
//! use actuate::prelude::*;
//!
//! #[data]
//! trait MyTrait: Data {
//!     fn run(&self);
//! }
//!
//! #[derive(Data)]
//! struct A<'a> {
//!     my_trait: Box<dyn MyTrait + 'a>,
//! }
//!
//! impl Compose for A<'_> {
//!     fn compose(cx: Scope<Self>) -> impl Compose {
//!         cx.me().my_trait.run();
//!     }
//! }
//!
//! #[derive(Data)]
//! struct X;
//!
//! impl MyTrait for X {
//!     fn run(&self) {
//!         dbg!("X");
//!     }
//! }
//!
//! #[derive(Data)]
//! struct App;
//!
//! impl Compose for App {
//!     fn compose(_cx: Scope<Self>) -> impl Compose {
//!         A {
//!             my_trait: Box::new(X),
//!         }
//!     }
//! }
//! ```

use crate::{HashMap, compose::DynCompose};
use core::{error::Error, future::Future, ops::Range, pin::Pin};

pub use actuate_macros::{Data, data};

/// Composable data.
///
/// In most cases, this trait should be derived with `#[derive(Data)]`.
/// For more information, see the [module-level documentation](crate::data).
///
/// # Safety
/// This struct must ensure the lifetime of the data it holds cannot escape while composing children.
///
/// For example, a `RefCell<&'a T>` is unsafe because the compiler will infer the lifetime of a child composable's lifetime (e.g. `'a`)
/// as this struct's lifetime (e.g. `'a`).
pub unsafe trait Data {}

macro_rules! impl_data_for_std {
    ($($t:ty),*) => {
        $(
            unsafe impl Data for $t {}
        )*
    }
}

impl_data_for_std!(
    (),
    bool,
    char,
    f32,
    f64,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    String
);

unsafe impl Data for &str {}

unsafe impl<T: Data> Data for Vec<T> {}

unsafe impl<T: Data, U: Data, S: 'static> Data for HashMap<T, U, S> {}

unsafe impl<T: 'static> Data for &T {}

unsafe impl<T: Data> Data for Option<T> {}

unsafe impl<T: Data, U: Data> Data for Result<T, U> {}

unsafe impl<T: Data> Data for Pin<T> {}

unsafe impl<T: 'static> Data for Range<T> {}

unsafe impl Data for Box<dyn Error> {}

unsafe impl Data for Box<dyn Future<Output = ()> + '_> {}

unsafe impl Data for DynCompose<'_> {}

#[doc(hidden)]
pub struct FieldWrap<T>(pub T);

#[doc(hidden)]
pub unsafe trait FnField<Marker> {
    fn check(&self) {
        let _ = self;
    }
}

macro_rules! impl_data_for_fns {
    ($($t:tt),*) => {
        unsafe impl<$($t: 'static,)* R: Data, F: Fn($($t,)*) -> R> FnField<fn($($t,)*)> for &FieldWrap<F> {}

        unsafe impl<$($t: 'static,)* R: Data> FnField<fn($($t,)*)> for &FieldWrap<alloc::rc::Rc<dyn Fn($($t,)*) -> R + '_>> {}
    }
}

impl_data_for_fns!();
impl_data_for_fns!(T1);
impl_data_for_fns!(T1, T2);
impl_data_for_fns!(T1, T2, T3);
impl_data_for_fns!(T1, T2, T3, T4);
impl_data_for_fns!(T1, T2, T3, T4, T5);
impl_data_for_fns!(T1, T2, T3, T4, T5, T6);
impl_data_for_fns!(T1, T2, T3, T4, T5, T6, T7);
impl_data_for_fns!(T1, T2, T3, T4, T5, T6, T7, T8);

#[doc(hidden)]
pub unsafe trait DataField {
    fn check(&self) {
        let _ = self;
    }
}

unsafe impl<T: Data> DataField for &FieldWrap<T> {}

#[doc(hidden)]
pub unsafe trait StaticField {
    fn check(&self) {
        let _ = self;
    }
}

unsafe impl<T: 'static> StaticField for &&FieldWrap<T> {}
