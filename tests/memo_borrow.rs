use actuate::{composer::Composer, prelude::*};
use std::{cell::Cell, rc::Rc};

/// A leaf that keeps queueing a recomposition of *its own node*, independent of any
/// parent. `SignalMut::update` queues the scope that owns the hook, and the composer
/// composes that one node in isolation, so this recomposes even while every ancestor
/// is memoized away.
#[derive(Data)]
struct Leaf {
    out: Rc<Cell<i32>>,
}

impl Compose for Leaf {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let n = use_mut(&cx, || 0i32);
        SignalMut::update(n, |n| *n += 1);

        cx.me().out.set(cx.me().out.get() + 1);
    }
}

/// Rebuilds its child on every recompose, so the previous `Memo` value -- and the
/// `Vec` allocation inside it -- is dropped on each pass.
#[derive(Data)]
struct MemoizedList {
    out: Rc<Cell<i32>>,
}

impl Compose for MemoizedList {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let n = use_mut(&cx, || 0i32);
        SignalMut::update(n, |n| *n += 1);

        // A fresh `Vec` allocation every recompose. `Vec<C>::compose` hands each
        // element node a raw pointer into whichever buffer it composed against.
        let items = vec![
            Leaf {
                out: cx.me().out.clone(),
            },
            Leaf {
                out: cx.me().out.clone(),
            },
        ];

        // The dependency never changes, so the `Vec` node is never re-composed and its
        // element pointers are never refreshed. They stay valid only because the node
        // owns the `Vec` it composed against -- this fresh one is dropped unused.
        memo((), items)
    }
}

/// Each pass the root recomposes and drops the previous `Memo` value, while the `Memo`
/// declines to re-compose the `Vec` node. A leaf, queued by its own state update, is
/// then composed through a pointer into that `Vec` -- which must still be alive.
#[test]
fn memo_survives_ancestor_recomposition() {
    let out = Rc::new(Cell::new(0));
    let mut composer = Composer::new(MemoizedList { out: out.clone() });

    for _ in 0..5 {
        let _ = composer.try_compose();
    }

    // The leaves kept composing against the memoized `Vec`.
    assert!(out.get() > 0);
}

/// The same shape without the `Memo`. Here the cascade reaches the `Vec` node, whose
/// `use_node` calls refresh every element pointer into the new buffer before any leaf
/// is composed -- so this is the control that isolates `Memo` as the cause.
#[derive(Data)]
struct PlainList {
    out: Rc<Cell<i32>>,
}

impl Compose for PlainList {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let n = use_mut(&cx, || 0i32);
        SignalMut::update(n, |n| *n += 1);

        vec![
            Leaf {
                out: cx.me().out.clone(),
            },
            Leaf {
                out: cx.me().out.clone(),
            },
        ]
    }
}

#[test]
fn plain_vec_refreshes_element_pointers() {
    let out = Rc::new(Cell::new(0));
    let mut composer = Composer::new(PlainList { out: out.clone() });

    for _ in 0..5 {
        let _ = composer.try_compose();
    }
}
