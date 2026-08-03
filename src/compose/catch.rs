use super::CatchContext;
use crate::{Scope, Signal, compose::Compose, data::Data, use_provider};
use alloc::rc::Rc;
use core::mem;

/// Create a composable that catches errors from its children.
/// This will catch all errors from its descendants, until another `catch` is encountered.
///
/// If a child returns a `Result<T, actuate::Error>`,
/// any errors will be caught by this composable by calling `on_error`.
///
/// # Examples
///
/// ```no_run
/// use actuate::prelude::*;
///
/// #[derive(Data)]
/// struct A;
///
/// impl Compose for A {
///     fn compose(_cx: Scope<Self>) -> impl Compose {
///         let _: i32 = "".parse().map_err(Error::new)?;
///
///         Ok(())
///     }
/// }
///
/// #[derive(Data)]
/// struct App;
///
/// impl Compose for App {
///     fn compose(_cx: Scope<Self>) -> impl Compose {
///         catch(
///             |error| {
///                 dbg!(error);
///             },
///             A,
///         )
///     }
/// }
/// ```
pub fn catch<'a, C: Compose>(
    on_error: impl Fn(Box<dyn core::error::Error>) + 'a,
    content: C,
) -> Catch<'a, C> {
    Catch {
        content,
        f: Rc::new(on_error),
    }
}

/// Error catch composable.
///
/// See [`catch`] for more.
#[derive(Clone, Data)]
#[actuate(path = "crate")]
pub struct Catch<'a, C> {
    /// Content of this composable.
    content: C,

    /// Function to handle errors.
    f: Rc<dyn Fn(Box<dyn core::error::Error>) + 'a>,
}

impl<C: Compose> Compose for Catch<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let f: &dyn Fn(Box<dyn core::error::Error>) = &*cx.me().f;

        // Cast this function to the `'static` lifetime.
        // Safety: This function has a lifetime of `'a`, which is guaranteed to outlive this composables descendants.
        let f: Rc<dyn Fn(Box<dyn core::error::Error>)> = unsafe { mem::transmute(f) };

        use_provider(&cx, move || CatchContext { f: f.clone() });

        // Safety: The content of this composable is only returned into the composition once.
        unsafe { Signal::map_unchecked(cx.me(), |me| &me.content) }
    }
}
