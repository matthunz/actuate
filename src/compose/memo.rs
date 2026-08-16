use super::{AnyCompose, Node, Runtime};
use crate::{Scope, ScopeData, compose::Compose, composer::ComposePtr, data::Data, use_ref};
use alloc::{borrow::Cow, rc::Rc};
use core::cell::{Cell, RefCell};
use slotmap::DefaultKey;

/// Create a new memoized composable.
///
/// The content of the memoized composable is only re-composed when the dependency changes.
///
/// Children of this `Memo` may still be re-composed if their state has changed.
///
/// # Borrowing
/// The content must be `'static`, so it cannot borrow from its ancestors.
///
/// Composing a node normally replaces every descendant's value before anything reads it,
/// which is what lets a child hold a reference into its parent. A `Memo` deliberately
/// stops that cascade, so its content outlives the composition of the ancestors that
/// built it -- any reference it held into them would dangle as soon as they re-composed.
///
/// To pass ancestor state through a `Memo`, clone it into the content and make it part of
/// the dependency:
///
/// ```
/// use actuate::prelude::*;
///
/// #[derive(Data)]
/// struct Child {
///     name: String,
/// }
///
/// impl Compose for Child {
///     fn compose(cx: Scope<Self>) -> impl Compose {}
/// }
///
/// #[derive(Data)]
/// struct App {
///     name: String,
/// }
///
/// impl Compose for App {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         let name = cx.me().name.clone();
///
///         memo(
///             name.clone(),
///             Child { name },
///         )
///     }
/// }
/// ```
pub fn memo<D, C>(dependency: D, content: C) -> Memo<D, C>
where
    D: Data + Clone + PartialEq + 'static,
    C: Compose + 'static,
{
    Memo {
        dependency,
        content: RefCell::new(Some(content)),
    }
}

/// Memoized composable.
///
/// See [`memo`] for more.
#[derive(Clone, Data)]
#[actuate(path = "crate")]
#[must_use = "Composables do nothing unless composed or returned from other composables."]
pub struct Memo<T, C: 'static> {
    dependency: T,
    /// The content built by the current composition, moved into the child node the first
    /// time this `Memo` composes and each time the dependency changes.
    ///
    /// The child node *owns* its content rather than pointing into this value. A parent
    /// re-composing replaces this whole `Memo` in place and drops the previous one, which
    /// would free anything the memoized subtree still points into -- the child node's
    /// pointers are only refreshed on the passes where it actually re-composes.
    content: RefCell<Option<C>>,
}

impl<T, C> Compose for Memo<T, C>
where
    T: Clone + Data + PartialEq + 'static,
    C: Compose + 'static,
{
    fn compose(cx: Scope<Self>) -> impl Compose {
        let rt = Runtime::current();

        let child_key: &Cell<Option<DefaultKey>> = use_ref(&cx, || Cell::new(None));
        let last: &RefCell<Option<T>> = use_ref(&cx, || RefCell::new(None));
        let mut last = last.borrow_mut();

        // The content built by this pass. It is dropped unused whenever the memo holds,
        // since the child node keeps the content it already owns.
        let content = cx.me().content.borrow_mut().take();

        if let Some(key) = child_key.get() {
            if let Some(last) = &*last
                && *last == cx.me().dependency
            {
                // The dependency is unchanged. Leave the child node untouched: it owns the
                // content it last composed with, so every pointer the subtree holds into
                // that value stays valid even though this `Memo` was just replaced.
                return;
            }

            *last = Some(cx.me().dependency.clone());

            if let Some(content) = content {
                let node = rt.nodes.borrow()[key].clone();

                let mut child: Box<dyn AnyCompose> = Box::new(content);

                // Swap the new content into the node's existing allocation so its address
                // stays stable and descendant pointers survive, then drop the old content.
                // The subtree is queued below, so it re-derives before anything reads it.
                //
                // Safety: the node was created below with a `C`, and this is a `C`.
                unsafe { child.reborrow(node.compose.borrow_mut().as_ptr_mut()) };
            }

            rt.queue(key);

            return;
        }

        // First composition: move the content into a node that owns it.
        let Some(content) = content else {
            return;
        };

        *last = Some(cx.me().dependency.clone());

        let mut nodes = rt.nodes.borrow_mut();

        let key = nodes.insert(Rc::new(Node {
            compose: RefCell::new(ComposePtr::Boxed(Box::new(content))),
            scope: ScopeData::default(),
            parent: Some(rt.current_key.get()),
            children: RefCell::new(Vec::new()),
            child_idx: 0,
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

        drop(nodes);

        child_key.set(Some(key));

        rt.queue(key);
    }

    fn name() -> Option<Cow<'static, str>> {
        Some(
            C::name()
                .map(|name| format!("Memo<{}>", name).into())
                .unwrap_or("Memo".into()),
        )
    }
}
