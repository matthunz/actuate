use super::{RuntimeContext, SpawnContext, SystemParamFunction, use_bundle_inner};
use crate::{
    Scope, Signal, compose::Compose, composer::Runtime, data::Data, use_context, use_drop,
    use_provider, use_ref,
};
use bevy_ecs::{entity::Entity, prelude::*, world::World};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    mem,
    rc::Rc,
    sync::{Arc, Mutex},
};

/// Create a [`Spawn`] composable that spawns the provided `bundle` when composed.
///
/// On re-composition, the spawned entity is updated to the latest provided value.
///
/// # Examples
///
/// ```no_run
/// use actuate::prelude::*;
/// use bevy::prelude::*;
///
/// #[derive(Data)]
/// struct Button {
///     label: String,
///     color: Color
/// }
///
/// impl Compose for Button {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         // Spawn an entity with a `Text` and `BackgroundColor` component.
///         spawn((Text::new(cx.me().label.clone()), BackgroundColor(cx.me().color)))
///     }
/// }
/// ```
pub fn spawn<'a, B>(bundle: B) -> Spawn<'a>
where
    B: Bundle + Clone,
{
    Spawn {
        spawn_fn: Rc::new(move |world, cell| {
            if let Some(entity) = cell {
                world.entity_mut(*entity).insert(bundle.clone());
            } else {
                *cell = Some(world.spawn(bundle.clone()).id())
            }
        }),
        content: (),
        target: None,
        observer_fns: Vec::new(),
        observer_guard: Arc::new(Mutex::new(true)),
        on_spawn: Vec::new(),
        on_insert: Vec::new(),
    }
}

type SpawnFn = Rc<dyn Fn(&mut World, &mut Option<Entity>)>;

type ObserverFn<'a> = Rc<dyn Fn(&mut EntityWorldMut) + 'a>;

type OnInsertFn<'a> = Rc<dyn Fn(EntityWorldMut) + 'a>;

/// Composable to spawn an entity.
///
/// See [`spawn`] for more information.
#[derive(Clone)]
#[must_use = "Composables do nothing unless composed or returned from other composables."]
pub struct Spawn<'a, C = ()> {
    spawn_fn: SpawnFn,
    content: C,
    target: Option<Entity>,
    observer_fns: Vec<ObserverFn<'a>>,
    on_spawn: Vec<OnInsertFn<'a>>,
    on_insert: Vec<OnInsertFn<'a>>,
    observer_guard: Arc<Mutex<bool>>,
}

impl<'a, C> Spawn<'a, C> {
    /// Set the target entity to spawn the composition into.
    ///
    /// If `None`, this will use the composition's parent (if any).
    pub fn target(mut self, target: Entity) -> Self {
        self.target = Some(target);
        self
    }

    /// Set the child content.
    pub fn content<C2>(self, content: C2) -> Spawn<'a, C2> {
        Spawn {
            spawn_fn: self.spawn_fn,
            content,
            target: self.target,
            observer_fns: self.observer_fns,
            observer_guard: Arc::new(Mutex::new(false)),
            on_spawn: self.on_spawn,
            on_insert: self.on_insert,
        }
    }

    /// Add a function to be called when this bundle is initially spawned.
    pub fn on_spawn(mut self, f: impl Fn(EntityWorldMut) + 'a) -> Self {
        self.on_insert.push(Rc::new(f));
        self
    }

    /// Add a function to be called on every insert.
    pub fn on_insert(mut self, f: impl Fn(EntityWorldMut) + 'a) -> Self {
        self.on_insert.push(Rc::new(f));
        self
    }

    /// Add an observer to the spawned entity.
    pub fn observe<F, E, B, Marker>(mut self, observer: F) -> Self
    where
        F: SystemParamFunction<Marker, In = On<'static, 'static, E, B>, Out = ()>
            + Send
            + Sync
            + 'a,
        E: EntityEvent,
        B: Bundle,
    {
        let cell = Cell::new(Some(observer));
        let guard = self.observer_guard.clone();

        self.observer_fns.push(Rc::new(move |entity| {
            let mut observer = cell.take().unwrap();
            let guard = guard.clone();

            type SpawnObserveFn<'a, F, E, B, Marker> = Box<
                dyn FnMut(
                        On<'_, '_, E, B>,
                        ParamSet<'_, '_, (<F as SystemParamFunction<Marker>>::Param,)>,
                    ) + Send
                    + Sync
                    + 'a,
            >;

            let f: SpawnObserveFn<'a, F, E, B, Marker> = Box::new(move |trigger, mut params| {
                let guard = guard.lock().unwrap();
                if !*guard {
                    panic!("Actuate observer called after its scope was dropped.")
                }

                // Safety: The event will be accessed under a shortened lifetime.
                let trigger: On<'static, 'static, E, B> = unsafe { mem::transmute(trigger) };
                observer.run(trigger, params.p0())
            });

            // Safety: The observer will be disabled after this scope is dropped.
            let f: SpawnObserveFn<'static, F, E, B, Marker> = unsafe { mem::transmute(f) };

            entity.observe(f);
        }));
        self
    }
}

unsafe impl<C: Data> Data for Spawn<'_, C> {}

impl<C: Compose> Compose for Spawn<'_, C> {
    fn compose(cx: Scope<Self>) -> impl Compose {
        let rt = Runtime::current();

        let spawn_cx = use_context::<SpawnContext>(&cx);

        let is_initial = use_ref(&cx, || Cell::new(true));
        let entity = use_bundle_inner(&cx, |world, entity| {
            if let Some(target) = cx.me().target {
                *entity = Some(target);
            }

            // Check if this entity has been removed externally.
            if let Some(entity) = entity
                && world.get_entity(*entity).is_err()
            {
                return;
            }

            (cx.me().spawn_fn)(world, entity);

            for f in &cx.me().on_insert {
                f(world.entity_mut(entity.unwrap()));
            }

            if is_initial.get() {
                for f in &cx.me().on_spawn {
                    f(world.entity_mut(entity.unwrap()));
                }

                let mut entity_mut = world.entity_mut(entity.unwrap());
                for f in &cx.me().observer_fns {
                    f(&mut entity_mut);
                }

                is_initial.set(false);
            }
        });
        let key = use_ref(&cx, || rt.pending(rt.current_key.get()));

        use_provider(&cx, || {
            if cx.me().target.is_none()
                && let Ok(spawn_cx) = spawn_cx
            {
                spawn_cx.keys.borrow_mut().insert(key.clone());

                if let Some(idx) = spawn_cx
                    .keys
                    .borrow()
                    .iter()
                    .position(|pending| pending.key == rt.current_key.get())
                {
                    let world = unsafe { RuntimeContext::current().world_mut() };
                    world
                        .entity_mut(spawn_cx.parent_entity)
                        .insert_children(idx, &[entity]);
                }
            }

            SpawnContext {
                parent_entity: entity,
                keys: RefCell::new(BTreeSet::new()),
            }
        });

        // Use the initial guard.
        let guard = use_ref(&cx, || cx.me().observer_guard.clone());
        use_drop(&cx, move || {
            *guard.lock().unwrap() = false;

            if let Ok(spawn_cx) = spawn_cx {
                spawn_cx.keys.borrow_mut().remove(key);
            }
        });

        unsafe { Signal::map_unchecked(cx.me(), |me| &me.content) }
    }
}
