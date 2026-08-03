use super::{AnyCompose, Runtime, use_node};
use crate::{Scope, compose::Compose, composer::ComposePtr, data::Data, use_ref};
use alloc::borrow::Cow;
use core::{cell::RefCell, mem};

/// Create a new memoized composable.
///
/// The content of the memoized composable is only re-composed when the dependency changes.
///
/// Children of this `Memo` may still be re-composed if their state has changed.
pub fn memo<D, C>(dependency: D, content: C) -> Memo<D, C>
where
    D: Data + Clone + PartialEq + 'static,
    C: Compose,
{
    Memo {
        dependency,
        content,
    }
}

/// Memoized composable.
///
/// See [`memo`] for more.
#[derive(Clone, Data)]
#[actuate(path = "crate")]
#[must_use = "Composables do nothing unless composed or returned from other composables."]
pub struct Memo<T, C> {
    dependency: T,
    content: C,
}

impl<T, C> Compose for Memo<T, C>
where
    T: Clone + Data + PartialEq + 'static,
    C: Compose,
{
    fn compose(cx: Scope<Self>) -> impl Compose {
        let rt = Runtime::current();

        let ptr: *const dyn AnyCompose =
            unsafe { mem::transmute(&cx.me().content as *const dyn AnyCompose) };
        let (key, _) = use_node(&cx, ComposePtr::Ptr(ptr), 0);

        let last = use_ref(&cx, RefCell::default);
        let mut last = last.borrow_mut();

        if let Some(last) = &mut *last {
            if cx.me().dependency != *last {
                *last = cx.me().dependency.clone();

                rt.queue(key);
            }
        } else {
            *last = Some(cx.me().dependency.clone());

            rt.queue(key);
        }
    }

    fn name() -> Option<Cow<'static, str>> {
        Some(
            C::name()
                .map(|name| format!("Memo<{}>", name).into())
                .unwrap_or("Memo".into()),
        )
    }
}
