use super::{AnyCompose, Node, Runtime};
use crate::{Scope, ScopeData, Signal, compose::Compose, data::Data, use_ref};
use alloc::rc::Rc;
use core::{
    cell::{RefCell, UnsafeCell},
    marker::PhantomData,
    mem,
    ptr::NonNull,
};
use slotmap::DefaultKey;

/// Create a composable from an iterator.
///
/// `make_item` will be called for each item to produce a composable.
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
///         compose::from_iter(0..10, |id| {
///             User { id: *id }
///         })
///     }
/// }
/// ```
pub fn from_iter<'a, I, C>(
    iter: I,
    make_item: impl Fn(Signal<'a, I::Item>) -> C + 'a,
) -> FromIter<'a, I, I::Item, C>
where
    I: IntoIterator + Clone + Data,
    I::Item: 'static,
    C: Compose,
{
    FromIter {
        iter,
        make_item: Rc::new(make_item),
    }
}

/// Composable from an iterator.
///
/// For more see [`from_iter`].
#[must_use = "Composables do nothing unless composed or returned from other composables."]
pub struct FromIter<'a, I, Item, C> {
    iter: I,
    make_item: Rc<dyn Fn(Signal<'a, Item>) -> C + 'a>,
}

impl<I, Item, C> Clone for FromIter<'_, I, Item, C>
where
    I: Clone,
    C: Clone,
{
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            make_item: self.make_item.clone(),
        }
    }
}

unsafe impl<I, Item, C> Data for FromIter<'_, I, Item, C>
where
    I: Data,
    Item: 'static,
    C: Data,
{
}

impl<I, Item, C> Compose for FromIter<'_, I, Item, C>
where
    I: IntoIterator<Item = Item> + Clone + Data,
    Item: 'static,
    C: Compose,
{
    fn compose(cx: Scope<Self>) -> impl Compose {
        let states: &RefCell<Vec<ItemState<Item>>> = use_ref(&cx, || RefCell::new(Vec::new()));
        let mut states = states.borrow_mut();

        let mut items: Vec<Option<_>> = cx.me().iter.clone().into_iter().map(Some).collect();

        let rt = Runtime::current();

        // Refresh the items that already have state, so existing children observe
        // the current value rather than the one they were created with.
        //
        // Safety: the item is stored in an `UnsafeCell`, so the references handed to
        // child composables stay valid across this write. No child is composing while
        // this composable runs, so no reference to the item is live here.
        for (idx, item) in items.iter_mut().enumerate().take(states.len()) {
            unsafe { *states[idx].item.get() = item.take().unwrap() };
        }

        if items.len() >= states.len() {
            for item in &mut items[states.len()..] {
                let item = item.take().unwrap();

                let state = ItemState {
                    item: Box::new(UnsafeCell::new(item)),
                    key: None,
                };
                states.push(state);
            }
        } else {
            states.truncate(items.len());
        }

        for idx in 0..states.len() {
            let mut nodes = rt.nodes.borrow_mut();

            if states[idx].key.is_none() {
                // Safety: `item` is boxed, so this pointer stays valid even after
                // `states` is reallocated by a later push. It is derived straight from
                // the `UnsafeCell` rather than through a reference, so that updating
                // the item in place does not invalidate it.
                let item_ptr = unsafe { NonNull::new_unchecked(states[idx].item.get()) };
                let compose = (cx.me().make_item)(Signal {
                    value: item_ptr,
                    generation: &cx.generation as _,
                    _marker: PhantomData,
                });
                let any_compose: Box<dyn AnyCompose> = Box::new(compose);
                let any_compose: Box<dyn AnyCompose> = unsafe { mem::transmute(any_compose) };

                let key = nodes.insert(Rc::new(Node {
                    compose: RefCell::new(crate::composer::ComposePtr::Boxed(any_compose)),
                    scope: ScopeData::default(),
                    parent: Some(rt.current_key.get()),
                    children: RefCell::new(Vec::new()),
                    child_idx: idx,
                }));
                nodes
                    .get(rt.current_key.get())
                    .unwrap()
                    .children
                    .borrow_mut()
                    .push(key);

                states[idx].key = Some(key);
            }

            let node = nodes.get(states[idx].key.unwrap()).unwrap().clone();

            *node.scope.contexts.borrow_mut() = cx.contexts.borrow().clone();
            node.scope
                .contexts
                .borrow_mut()
                .values
                .extend(cx.child_contexts.borrow().values.clone());

            drop(nodes);

            rt.queue(states[idx].key.unwrap());
        }
    }
}

struct ItemState<T> {
    /// Boxed so that child composables can hold a stable reference to this item
    /// across reallocations of the enclosing `Vec<ItemState<T>>`, and wrapped in an
    /// `UnsafeCell` so that those references survive the item being updated in place.
    item: Box<UnsafeCell<T>>,
    key: Option<DefaultKey>,
}
