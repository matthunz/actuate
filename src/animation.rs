use crate::{
    ScopeState, Signal, SignalMut,
    data::Data,
    ecs::{use_world, use_world_once},
    use_local_task, use_mut, use_ref,
};
use bevy_ecs::prelude::*;
use bevy_math::VectorSpace;
use bevy_time::Time;
use std::{
    cell::{Cell, RefCell},
    ops::Deref,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

struct State<T> {
    from: T,
    to: T,
    duration: Duration,
    tx: Option<oneshot::Sender<()>>,
}

/// Use an animated value.
pub fn use_animated<T>(cx: ScopeState<'_>, make_initial: impl FnOnce() -> T) -> UseAnimated<'_, T>
where
    T: VectorSpace + Send + 'static,
    T::Scalar: From<f32>,
{
    let start_cell = use_world_once(cx, |time: Res<Time>| Cell::new(Some(time.elapsed_secs())));

    let (controller, rx) = use_ref(cx, || {
        let (tx, rx) = mpsc::unbounded_channel();
        (AnimationController { tx }, Cell::new(Some(rx)))
    });

    let state: &RefCell<Option<State<T>>> = use_ref(cx, || RefCell::new(None));

    let out = use_mut(cx, make_initial);

    let time_cell = use_ref(cx, || Cell::new(start_cell.get().unwrap()));
    use_world(cx, |time_res: Res<Time>| {
        time_cell.set(time_res.elapsed_secs());
    });

    use_local_task(cx, move || async move {
        let mut rx = rx.take().unwrap();
        while let Some((to, duration, tx)) = rx.recv().await {
            *state.borrow_mut() = Some(State {
                from: *out,
                to,
                duration,
                tx: Some(tx),
            });
            start_cell.set(Some(time_cell.get()));
        }
    });

    use_world(cx, move |time: Res<Time>| {
        if let Some(start) = start_cell.get() {
            let mut state_cell = state.borrow_mut();
            if let Some(state) = &mut *state_cell {
                let elapsed = time.elapsed_secs() - start;

                if elapsed < state.duration.as_secs_f32() {
                    SignalMut::set(
                        out,
                        state.from.lerp(
                            state.to,
                            T::Scalar::from(elapsed / state.duration.as_secs_f32()),
                        ),
                    );
                } else {
                    SignalMut::set(out, state.to);
                    state.tx.take().unwrap().send(()).unwrap();
                    *state_cell = None;
                }
            }
        }
    });

    UseAnimated {
        value: SignalMut::as_ref(out),
        controller,
    }
}

/// Hook for [`use_animated`].
pub struct UseAnimated<'a, T> {
    value: Signal<'a, T>,
    controller: &'a AnimationController<T>,
}

impl<T> UseAnimated<'_, T> {
    /// Animate this value over a duration.
    pub async fn animate(&self, to: T, duration: Duration) {
        self.controller.animate(to, duration).await
    }

    /// Get the controller for this animation.
    pub fn controller(&self) -> AnimationController<T> {
        self.controller.clone()
    }
}

impl<T> Clone for UseAnimated<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for UseAnimated<'_, T> {}

impl<T> Deref for UseAnimated<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

unsafe impl<T> Data for UseAnimated<'_, T> {}

/// Controller for an animation created with [`use_animated`].
pub struct AnimationController<T> {
    tx: mpsc::UnboundedSender<(T, Duration, oneshot::Sender<()>)>,
}

impl<T> AnimationController<T> {
    /// Animate this value over a duration.
    pub async fn animate(&self, to: T, duration: Duration) {
        let (tx, rx) = oneshot::channel();
        self.tx.send((to, duration, tx)).unwrap();
        rx.await.unwrap()
    }
}

impl<T> Clone for AnimationController<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}
