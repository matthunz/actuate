use crate::{Data, Scope, ScopeState, compose::Compose};
use core::marker::PhantomData;

/// Create a composable from a function.
///
/// This will create a composable from a function that takes a [`ScopeState`] and returns some composable content.
///
/// # Examples
///
/// ```
/// use actuate::prelude::*;
///
/// #[derive(Data)]
/// struct User {
///     id: i32,
/// }
///
/// impl Compose for User {
///     fn compose(cx: Scope<Self>) -> impl Compose {}
/// }
///
/// #[derive(Data)]
/// struct App;
///
/// impl Compose for App {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         compose::from_fn(|_cx| {
///             User { id: 0 }
///         })
///     }
/// }
/// ```
pub fn from_fn<F, C>(f: F) -> FromFn<F, C>
where
    F: Fn(ScopeState) -> C,
    C: Compose,
{
    FromFn {
        f,
        _marker: PhantomData,
    }
}

/// Function composable.
///
/// For more see [`from_fn`].
pub struct FromFn<F, C> {
    f: F,
    _marker: PhantomData<C>,
}

impl<F: Clone, C> Clone for FromFn<F, C> {
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            _marker: PhantomData,
        }
    }
}

unsafe impl<F, C> Data for FromFn<F, C>
where
    F: Fn(ScopeState) -> C,
    C: Compose,
{
}

impl<F, C> Compose for FromFn<F, C>
where
    F: Fn(ScopeState) -> C,
    C: Compose,
{
    fn compose(cx: Scope<Self>) -> impl Compose {
        (cx.me().f)(&cx)
    }
}
