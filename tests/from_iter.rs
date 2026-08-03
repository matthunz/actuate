use actuate::{composer::Composer, prelude::*};
use std::{cell::RefCell, rc::Rc};

/// Records the value it sees on every compose.
#[derive(Data)]
struct Recorder<'a> {
    value: Signal<'a, i32>,
    seen: Rc<RefCell<Vec<i32>>>,
}

impl Compose for Recorder<'_> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        cx.me().seen.borrow_mut().push(*cx.me().value);
    }
}

/// A `from_iter` over a *fixed-length* list whose element values change.
#[derive(Data)]
struct App {
    seen: Rc<RefCell<Vec<i32>>>,
}

impl Compose for App {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let n = use_mut(&cx, || 0);

        if *n < 3 {
            SignalMut::update(n, |n| *n += 1);
        }

        let seen = cx.me().seen.clone();
        compose::from_iter(vec![*n * 10], move |value| Recorder {
            value,
            seen: seen.clone(),
        })
    }
}

#[test]
fn from_iter_tracks_changed_items() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut composer = Composer::new(App { seen: seen.clone() });

    for _ in 0..20 {
        let _ = composer.try_compose();
    }

    let seen = seen.borrow();
    println!("child saw: {seen:?}");

    // The parent's list goes 0 -> 10 -> 20 -> 30, so the child must observe
    // the updated value, not just the one it was first created with.
    assert!(
        seen.contains(&30),
        "child never saw the updated item; it saw {seen:?}"
    );
}
