use crate::{
    ScopeData,
    compose::{AnyCompose, CatchContext, Compose},
};
use alloc::{collections::BTreeSet, rc::Rc, sync::Arc, task::Wake};
use core::{
    any::TypeId,
    cell::{Cell, RefCell},
    cmp::Ordering,
    error::Error,
    fmt,
    future::Future,
    mem,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use crossbeam_queue::SegQueue;
use slotmap::{DefaultKey, SlotMap};

#[cfg(feature = "executor")]
use tokio::sync::RwLock;

type RuntimeFuture = Pin<Box<dyn Future<Output = ()>>>;

pub(crate) enum ComposePtr {
    Boxed(Box<dyn AnyCompose>),
    Ptr(*const dyn AnyCompose),
}

impl AnyCompose for ComposePtr {
    fn data_id(&self) -> TypeId {
        match self {
            ComposePtr::Boxed(compose) => compose.data_id(),
            ComposePtr::Ptr(ptr) => unsafe { (**ptr).data_id() },
        }
    }

    fn as_ptr_mut(&mut self) -> *mut () {
        match self {
            ComposePtr::Boxed(compose) => compose.as_ptr_mut(),
            ComposePtr::Ptr(ptr) => *ptr as *mut (),
        }
    }

    unsafe fn reborrow(&mut self, ptr: *mut ()) {
        match self {
            // Safety: the caller guarantees `ptr` is valid for this compose.
            ComposePtr::Boxed(compose) => unsafe { compose.reborrow(ptr) },
            ComposePtr::Ptr(_) => {}
        }
    }

    unsafe fn any_compose(&self, state: &ScopeData) {
        match self {
            // Safety: the caller guarantees `&self` is valid for the lifetime of `state`.
            ComposePtr::Boxed(compose) => unsafe { compose.any_compose(state) },
            // Safety: the caller guarantees `&self`, and therefore `ptr`,
            // is valid for the lifetime of `state`.
            ComposePtr::Ptr(ptr) => unsafe { (**ptr).any_compose(state) },
        }
    }

    fn name(&self) -> Option<std::borrow::Cow<'static, str>> {
        match self {
            ComposePtr::Boxed(compose) => compose.name(),
            ComposePtr::Ptr(ptr) => unsafe { (**ptr).name() },
        }
    }
}

// Safety: `scope` must be dropped before `compose`.
pub(crate) struct Node {
    pub(crate) compose: RefCell<ComposePtr>,
    pub(crate) scope: ScopeData<'static>,
    pub(crate) parent: Option<DefaultKey>,
    pub(crate) children: RefCell<Vec<DefaultKey>>,
    pub(crate) child_idx: usize,
}

/// Runtime for a [`Composer`].
#[derive(Clone)]
pub(crate) struct Runtime {
    /// Local task stored on this runtime.
    pub(crate) tasks: Rc<RefCell<SlotMap<DefaultKey, RuntimeFuture>>>,

    /// Queue for ready local tasks.
    pub(crate) task_queue: Arc<SegQueue<DefaultKey>>,

    /// Queue for updates that mutate the composition tree.
    pub(crate) update_queue: Rc<SegQueue<Box<dyn FnMut()>>>,

    #[cfg(feature = "executor")]
    /// Update lock for shared tasks.
    pub(crate) lock: Arc<RwLock<()>>,

    pub(crate) waker: RefCell<Option<Waker>>,

    pub(crate) nodes: Rc<RefCell<SlotMap<DefaultKey, Rc<Node>>>>,

    pub(crate) current_key: Rc<Cell<DefaultKey>>,

    pub(crate) root: DefaultKey,

    pub(crate) pending: Rc<RefCell<BTreeSet<Pending>>>,
}

impl Runtime {
    /// Get the current [`Runtime`].
    ///
    /// # Panics
    /// Panics if called outside of a runtime.
    pub fn current() -> Self {
        RUNTIME.with(|runtime| {
            runtime
                .borrow()
                .as_ref()
                .expect("Runtime::current() called outside of a runtime")
                .clone()
        })
    }

    /// Enter this runtime, making it available to [`Runtime::current`].
    pub fn enter(&self) {
        RUNTIME.with(|runtime| {
            *runtime.borrow_mut() = Some(self.clone());
        });
    }

    /// Queue an update to run after [`Composer::compose`].
    pub fn update(&self, f: impl FnOnce() + Send + 'static) {
        let mut f_cell = Some(f);

        #[cfg(feature = "executor")]
        let lock = self.lock.clone();

        self.update_queue.push(Box::new(move || {
            #[cfg(feature = "executor")]
            let _guard = lock.blocking_write();

            let f = f_cell.take().unwrap();
            f()
        }));

        if let Some(waker) = &*self.waker.borrow() {
            waker.wake_by_ref();
        }
    }

    pub fn pending(&self, key: DefaultKey) -> Pending {
        let nodes = self.nodes.borrow();
        let node = nodes[key].clone();

        let mut indices = vec![node.child_idx];
        let mut parent = node.parent;

        while let Some(key) = parent {
            indices.push(nodes.get(key).unwrap().child_idx);
            parent = nodes.get(key).unwrap().parent;
        }

        indices.reverse();

        Pending { key, indices }
    }

    pub fn queue(&self, key: DefaultKey) {
        let pending = self.pending(key);
        self.pending.borrow_mut().insert(pending);
    }
}

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

struct TaskWaker {
    key: DefaultKey,
    queue: Arc<SegQueue<DefaultKey>>,
    waker: Option<Waker>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.queue.push(self.key);
        if let Some(waker) = self.waker.as_ref() {
            waker.wake_by_ref();
        }
    }
}

/// Error for [`Composer::try_compose`].
#[derive(Debug)]
pub enum TryComposeError {
    /// No updates are ready to be applied.
    Pending,

    /// An error occurred during composition.
    Error(Box<dyn Error>),
}

impl PartialEq for TryComposeError {
    fn eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Pending {
    pub(crate) key: DefaultKey,
    pub(crate) indices: Vec<usize>,
}

impl PartialOrd for Pending {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pending {
    fn cmp(&self, other: &Self) -> Ordering {
        for (a, b) in self.indices.iter().zip(other.indices.iter()) {
            match a.cmp(b) {
                Ordering::Equal => {}
                x => return x,
            }
        }

        self.indices.len().cmp(&other.indices.len())
    }
}

/// Composer for composable content.
///
/// ```
/// use actuate::prelude::*;
/// use actuate::composer::Composer;
///
/// #[derive(Data)]
/// struct A;
///
/// impl Compose for A {
///     fn compose(cx: Scope<Self>) -> impl Compose {
///         (B, C)
///     }
/// }
///
/// #[derive(Data)]
/// struct B;
///
/// impl Compose for B {
///     fn compose(cx: Scope<Self>) -> impl Compose {}
/// }
///
/// #[derive(Data)]
/// struct C;
///
/// impl Compose for C {
///     fn compose(cx: Scope<Self>) -> impl Compose {}
/// }
///
/// let mut composer = Composer::new(A);
/// composer.try_compose().unwrap();
///
/// assert_eq!(format!("{:?}", composer), "Composer(A(B, C))")
/// ```
pub struct Composer {
    rt: Runtime,
    task_queue: Arc<SegQueue<DefaultKey>>,
    update_queue: Rc<SegQueue<Box<dyn FnMut()>>>,
    is_initial: bool,
}

impl Composer {
    /// Create a new [`Composer`] with the given content, updater, and task executor.
    pub fn new(content: impl Compose + 'static) -> Self {
        #[cfg(feature = "executor")]
        let lock = Arc::new(RwLock::new(()));

        let task_queue = Arc::new(SegQueue::new());
        let update_queue = Rc::new(SegQueue::new());

        let mut nodes = SlotMap::new();
        let root_key = nodes.insert(Rc::new(Node {
            compose: RefCell::new(ComposePtr::Boxed(Box::new(content))),
            scope: ScopeData::default(),
            parent: None,
            children: RefCell::new(Vec::new()),
            child_idx: 0,
        }));

        Self {
            rt: Runtime {
                tasks: Rc::new(RefCell::new(SlotMap::new())),
                task_queue: task_queue.clone(),
                update_queue: update_queue.clone(),
                waker: RefCell::new(None),
                #[cfg(feature = "executor")]
                lock,
                nodes: Rc::new(RefCell::new(nodes)),
                current_key: Rc::new(Cell::new(root_key)),
                root: root_key,
                pending: Rc::new(RefCell::new(BTreeSet::new())),
            },
            task_queue,
            update_queue,
            is_initial: true,
        }
    }

    /// Returns `true` if this composer has work ready to run.
    ///
    /// This is intended for backends that only drive the composer when there is something
    /// to do, rather than on every frame.
    ///
    /// [`Composer::poll_compose`] returning [`Poll::Pending`] does *not* mean the composer
    /// is idle: applying a queued update can itself queue a recomposition, and that happens
    /// on the way out of a composition pass. Use this method to decide whether to schedule
    /// another pass.
    ///
    /// Call this between passes; it borrows state that is mutably borrowed while composing.
    pub fn is_ready(&self) -> bool {
        self.is_initial
            || !self.rt.pending.borrow().is_empty()
            || !self.task_queue.is_empty()
            || !self.update_queue.is_empty()
    }

    /// Try to immediately compose the content in this composer.
    pub fn try_compose(&mut self) -> Result<(), TryComposeError> {
        let mut is_pending = true;

        for res in self.by_ref() {
            res.map_err(TryComposeError::Error)?;

            is_pending = false;
        }

        if is_pending {
            Err(TryComposeError::Pending)
        } else {
            Ok(())
        }
    }

    /// Poll a composition of the content in this composer.
    pub fn poll_compose(&mut self, cx: &mut Context) -> Poll<Result<(), Box<dyn Error>>> {
        *self.rt.waker.borrow_mut() = Some(cx.waker().clone());

        match self.try_compose() {
            Ok(()) => Poll::Ready(Ok(())),
            Err(TryComposeError::Pending) => Poll::Pending,
            Err(TryComposeError::Error(error)) => Poll::Ready(Err(error)),
        }
    }

    /// Compose the content of this composer.
    pub async fn compose(&mut self) -> Result<(), Box<dyn Error>> {
        futures::future::poll_fn(|cx| self.poll_compose(cx)).await
    }
}

impl Drop for Composer {
    fn drop(&mut self) {
        let node = self.rt.nodes.borrow()[self.rt.root].clone();
        drop_recursive(&self.rt, self.rt.root, node)
    }
}

fn drop_recursive(rt: &Runtime, key: DefaultKey, node: Rc<Node>) {
    let children = node.children.borrow().clone();
    for child_key in children {
        let child = rt.nodes.borrow()[child_key].clone();
        drop_recursive(rt, child_key, child)
    }

    rt.nodes.borrow_mut().remove(key);
}

impl Iterator for Composer {
    type Item = Result<(), Box<dyn Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.rt.enter();

        let error_cell = Rc::new(Cell::new(None));
        let error_cell_handle = error_cell.clone();

        let root = self.rt.nodes.borrow().get(self.rt.root).unwrap().clone();
        root.scope.contexts.borrow_mut().values.insert(
            TypeId::of::<CatchContext>(),
            Rc::new(CatchContext::new(move |error| {
                error_cell_handle.set(Some(error));
            })),
        );

        if !self.is_initial {
            let key_cell = self.rt.pending.borrow_mut().pop_first();
            if let Some(pending) = key_cell {
                self.rt.current_key.set(pending.key);

                let node = self.rt.nodes.borrow().get(pending.key).unwrap().clone();

                // Safety: `self.compose` is guaranteed to live as long as `self.scope_state`.
                unsafe { node.compose.borrow().any_compose(&node.scope) };
            } else {
                while let Some(key) = self.task_queue.pop() {
                    let waker = Waker::from(Arc::new(TaskWaker {
                        key,
                        waker: self.rt.waker.borrow().clone(),
                        queue: self.rt.task_queue.clone(),
                    }));
                    let mut cx = Context::from_waker(&waker);

                    let mut tasks = self.rt.tasks.borrow_mut();
                    let task = tasks.get_mut(key).unwrap();
                    let _ = task.as_mut().poll(&mut cx);
                }

                while let Some(mut update) = self.update_queue.pop() {
                    update();
                }

                return None;
            }
        } else {
            self.is_initial = false;

            self.rt.current_key.set(self.rt.root);

            // Safety: `self.compose` is guaranteed to live as long as `self.scope_state`.
            unsafe { root.compose.borrow().any_compose(&root.scope) };
        }

        Some(error_cell.take().map(Err).unwrap_or(Ok(())))
    }
}

impl fmt::Debug for Composer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut dbg_tuple = f.debug_tuple("Composer");

        dbg_composer(&mut dbg_tuple, &self.rt.nodes.borrow(), self.rt.root);

        dbg_tuple.finish()
    }
}

struct Field<'a> {
    name: &'a str,
    nodes: &'a SlotMap<DefaultKey, Rc<Node>>,
    children: &'a [DefaultKey],
}

impl fmt::Debug for Field<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut dbg_tuple = f.debug_tuple(self.name);

        for child_key in self.children {
            dbg_composer(&mut dbg_tuple, self.nodes, *child_key);
        }

        dbg_tuple.finish()
    }
}

fn dbg_composer(
    dbg_tuple: &mut fmt::DebugTuple,
    nodes: &SlotMap<DefaultKey, Rc<Node>>,
    key: DefaultKey,
) {
    let node = &nodes[key];
    if let Some(name) = node.compose.borrow().name() {
        dbg_tuple.field(&Field {
            name: &name,
            nodes,
            children: &node.children.borrow(),
        });
    } else {
        for child_key in &*node.children.borrow() {
            dbg_composer(dbg_tuple, nodes, *child_key);
        }
    }
}
