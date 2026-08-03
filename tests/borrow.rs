use actuate::{composer::Composer, prelude::*};
use std::{cell::Cell, rc::Rc};

// A child that *borrows* its item from the parent and dereferences it on every compose.
#[derive(Data)]
struct Item<'a> {
    id: Signal<'a, i32>,
    out: Rc<Cell<i32>>,
}

impl Compose for Item<'_> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        // Dereference the borrowed item.
        cx.me().out.set(*cx.me().id);
    }
}

/// A `from_iter` list whose length grows across recomposes.
#[derive(Data)]
struct GrowingList {
    out: Rc<Cell<i32>>,
}

impl Compose for GrowingList {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let n = use_mut(&cx, || 1i32);

        if *n < 8 {
            SignalMut::update(n, |n| *n += 1);
        }

        let out = cx.me().out.clone();
        compose::from_iter(0..*n, move |id| Item {
            id,
            out: out.clone(),
        })
    }
}

#[test]
fn from_iter_growing_list() {
    let out = Rc::new(Cell::new(0));
    let mut composer = Composer::new(GrowingList { out: out.clone() });

    for _ in 0..40 {
        let _ = composer.try_compose();
    }
}

/// A `Vec<C>` whose length shrinks across recomposes.
#[derive(Data)]
struct ShrinkingVec {
    out: Rc<Cell<i32>>,
}

impl Compose for ShrinkingVec {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let n = use_mut(&cx, || 8i32);

        if *n > 1 {
            SignalMut::update(n, |n| *n -= 1);
        }

        (0..*n)
            .map(|_| Counter {
                out: cx.me().out.clone(),
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Data)]
struct Counter {
    out: Rc<Cell<i32>>,
}

impl Compose for Counter {
    fn compose(cx: Scope<Self>) -> impl Compose {
        cx.me().out.set(cx.me().out.get() + 1);
    }
}

#[test]
fn vec_shrinking_list() {
    let out = Rc::new(Cell::new(0));
    let mut composer = Composer::new(ShrinkingVec { out: out.clone() });

    for _ in 0..40 {
        let _ = composer.try_compose();
    }
}
