use super::{AnyCompose, Node, Runtime};
use crate::{Scope, ScopeData, Signal, compose::Compose, data::Data, use_ref};
use alloc::rc::Rc;
use core::{cell::RefCell, mem};
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

        if items.len() >= states.len() {
            for item in &mut items[states.len()..] {
                let item = item.take().unwrap();

                let state = ItemState {
                    item: Box::new(item),
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
                // Safety: `item` is boxed, so this reference remains valid even after
                // `states` is reallocated by a later push.
                let item_ref: &Item = &states[idx].item;
                let item_ref: &Item = unsafe { mem::transmute(item_ref) };
                let compose = (cx.me().make_item)(Signal {
                    value: item_ref,
                    generation: &cx.generation as _,
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
    /// across reallocations of the enclosing `Vec<ItemState<T>>`.
    item: Box<T>,
    key: Option<DefaultKey>,
}
