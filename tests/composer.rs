use actuate::{
    composer::{Composer, TryComposeError},
    prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Data)]
struct Counter {
    x: Rc<Cell<i32>>,
}

impl Compose for Counter {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let updater = use_mut(&cx, || ());
        SignalMut::set(updater, ());

        cx.me().x.set(cx.me().x.get() + 1);
    }
}

#[derive(Data)]
struct NonUpdateCounter {
    x: Rc<Cell<i32>>,
}

impl Compose for NonUpdateCounter {
    fn compose(cx: Scope<Self>) -> impl Compose {
        cx.me().x.set(cx.me().x.get() + 1);
    }
}

#[test]
fn it_composes() {
    #[derive(Data)]
    struct Wrap {
        x: Rc<Cell<i32>>,
    }

    impl Compose for Wrap {
        fn compose(cx: Scope<Self>) -> impl Compose {
            Counter {
                x: cx.me().x.clone(),
            }
        }
    }

    let x = Rc::new(Cell::new(0));
    let mut composer = Composer::new(Wrap { x: x.clone() });

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 1);

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 2);
}

#[test]
fn it_composes_depth_first() {
    let a = Rc::new(Cell::new(0));
    let out = a.clone();

    let mut composer = Composer::new(compose::from_fn(move |_| {
        a.set(0);

        let b = a.clone();
        let e = a.clone();

        (
            compose::from_fn(move |_| {
                b.set(1);

                let c = b.clone();
                let d = b.clone();

                (
                    compose::from_fn(move |_| c.set(2)),
                    compose::from_fn(move |_| d.set(3)),
                )
            }),
            compose::from_fn(move |_| {
                e.set(4);

                let f = e.clone();
                let g = e.clone();

                (
                    compose::from_fn(move |_| f.set(5)),
                    compose::from_fn(move |_| g.set(6)),
                )
            }),
        )
    }));

    composer.next().unwrap().unwrap();
    assert_eq!(out.get(), 0);

    // Compose (1, 4)
    composer.next().unwrap().unwrap();

    composer.next().unwrap().unwrap();
    assert_eq!(out.get(), 1);

    // Compose (2, 3)
    composer.next().unwrap().unwrap();
    composer.next().unwrap().unwrap();
    assert_eq!(out.get(), 2);

    composer.next().unwrap().unwrap();
    assert_eq!(out.get(), 3);

    composer.next().unwrap().unwrap();
    assert_eq!(out.get(), 4);

    // Compose (5, 6)
    composer.next().unwrap().unwrap();

    composer.next().unwrap().unwrap();
    assert_eq!(out.get(), 5);

    composer.next().unwrap().unwrap();
    assert_eq!(out.get(), 6);
}

#[test]
fn it_skips_recomposes() {
    #[derive(Data)]
    struct Wrap {
        x: Rc<Cell<i32>>,
    }

    impl Compose for Wrap {
        fn compose(cx: Scope<Self>) -> impl Compose {
            NonUpdateCounter {
                x: cx.me().x.clone(),
            }
        }
    }

    let x = Rc::new(Cell::new(0));
    let mut composer = Composer::new(Wrap { x: x.clone() });

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 1);

    assert_eq!(composer.try_compose(), Err(TryComposeError::Pending));
    assert_eq!(x.get(), 1);
}

#[test]
fn it_composes_dyn_compose() {
    #[derive(Data)]
    struct Wrap {
        x: Rc<Cell<i32>>,
    }

    impl Compose for Wrap {
        fn compose(cx: crate::Scope<Self>) -> impl Compose {
            dyn_compose(Counter {
                x: cx.me().x.clone(),
            })
        }
    }

    let x = Rc::new(Cell::new(0));
    let mut composer = Composer::new(Wrap { x: x.clone() });

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 1);

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 2);
}

#[test]
fn it_composes_from_iter() {
    #[derive(Data)]
    struct Wrap {
        x: Rc<Cell<i32>>,
    }

    impl Compose for Wrap {
        fn compose(cx: crate::Scope<Self>) -> impl Compose {
            compose::from_iter(0..2, move |_| Counter {
                x: cx.me().x.clone(),
            })
        }
    }

    let x = Rc::new(Cell::new(0));
    let mut composer = Composer::new(Wrap { x: x.clone() });

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 2);

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 4);
}

#[test]
fn it_composes_memo() {
    #[derive(Data)]
    struct B {
        x: Rc<RefCell<i32>>,
    }

    impl Compose for B {
        fn compose(cx: Scope<Self>) -> impl Compose {
            *cx.me().x.borrow_mut() += 1;
        }
    }

    #[derive(Data)]
    struct A {
        x: Rc<RefCell<i32>>,
    }

    impl Compose for A {
        fn compose(cx: Scope<Self>) -> impl Compose {
            let x = cx.me().x.clone();
            memo((), B { x })
        }
    }

    let x = Rc::new(RefCell::new(0));
    let mut composer = Composer::new(A { x: x.clone() });

    composer.try_compose().unwrap();
    assert_eq!(*x.borrow(), 1);

    assert_eq!(composer.try_compose(), Err(TryComposeError::Pending));
    assert_eq!(*x.borrow(), 1);
}

#[test]
fn it_reports_readiness_after_applying_updates() {
    #[derive(Data)]
    struct Wrap {
        x: Rc<Cell<i32>>,
    }

    impl Compose for Wrap {
        fn compose(cx: Scope<Self>) -> impl Compose {
            NonUpdateCounter {
                x: cx.me().x.clone(),
            }
        }
    }

    let x = Rc::new(Cell::new(0));
    let mut composer = Composer::new(Wrap { x: x.clone() });

    // Nothing has been composed yet, so there is work to do.
    assert!(composer.is_ready());

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 1);

    // The tree is quiescent: nothing pending, no tasks, no updates.
    assert!(!composer.is_ready());
    assert_eq!(composer.try_compose(), Err(TryComposeError::Pending));
}

#[test]
fn it_is_ready_when_a_pending_update_queues_a_recompose() {
    // An update is applied on the way *out* of a composition pass, and applying it
    // queues a recomposition. So `try_compose` can report `Pending` while leaving work
    // queued -- backends must use `is_ready`, not the poll result, to decide whether to
    // schedule another pass.
    #[derive(Data)]
    struct Wrap {
        x: Rc<Cell<i32>>,
    }

    impl Compose for Wrap {
        fn compose(cx: Scope<Self>) -> impl Compose {
            Counter {
                x: cx.me().x.clone(),
            }
        }
    }

    let x = Rc::new(Cell::new(0));
    let mut composer = Composer::new(Wrap { x: x.clone() });

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 1);

    // `Counter` calls `SignalMut::set` on every compose, so a recomposition is always
    // queued by the time the pass finishes.
    assert!(
        composer.is_ready(),
        "an applied update queued a recompose, so the composer must report ready"
    );

    composer.try_compose().unwrap();
    assert_eq!(x.get(), 2);
}
