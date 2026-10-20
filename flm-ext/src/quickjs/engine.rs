//! The engine instances: one QuickJS runtime with one context per host, per
//! loaded extension, per thread, created on first use and kept in a
//! thread-local map.
//!
//! An instance never leaves its thread (rquickjs's runtime is not `Send`
//! without its `parallel` feature). [`with_engine`] is the one way into an
//! instance; a nested entry into an instance already running a handler on
//! this thread reuses the live context (`Ctx::from_raw`, the one `unsafe`
//! call of this module), since `Context::with` cannot be entered twice.
//!
//! An instance is created ([`Engine::create`]) in one of two modes: the
//! files mode, on the loading thread, reading the extension's modules
//! through its files; the recorded mode, everywhere else, from the modules
//! the load recorded ([`modules`](super::modules)). The outermost entry
//! into an instance ends by running the queued promise jobs
//! ([`Engine::drain`]): a refused `import()` is only met then, and so is a
//! promise rejected with no handler (recorded by the runtime's promise
//! rejection tracker); either fails the entry.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use rquickjs::{
    CatchResultExt, CaughtError, Coerced, Context, Ctx, Exception, Function, Module, Object, Persistent, Runtime,
    Value,
};
use serde_json::json;

use super::host::JsHost;
use super::modules::{FreeRefusals, ModuleLoader, ModuleResolver, ModuleState, SharedState, ENTRY, ENTRY_JS};
use super::package::JsExtension;
use super::{glue, OUT_OF_MEMORY_MESSAGE};
use crate::adapter::handles::CallTable;
use crate::binding::frames::{self, Frame};
use crate::binding::{self, Registered, Registration};
use crate::engine::ExtensionIdentity;
use super::memory::Limited;
use crate::paths;
use crate::runtime::ExtError;
use crate::source::ExtensionFiles;

/// The promises of an engine instance rejected with no handler, as the
/// runtime's promise rejection tracker records them: each promise, kept until
/// a handler is attached to it or the outermost entry ends, with the text of
/// its rejection's reason and whether that reason is the memory limit
/// reached (read as [`render_failure`] reads a failure). Not part of [`ModuleState`]: the promises must be
/// freed before the runtime is, which an [`Engine`] field declared before the
/// runtime, and the tracker the runtime clears before freeing itself, both
/// ensure.
type Rejections = Rc<RefCell<Vec<Rejected>>>;

/// One promise rejected with no handler: the promise, the text of its
/// rejection's reason, and whether the reason is the memory limit reached.
type Rejected = (Persistent<Value<'static>>, String, bool);

/// The sandbox script, evaluated once per engine instance before the prelude.
const SANDBOX_JS: &str = include_str!("js/sandbox.js");

/// The key of an engine instance on a thread: one per host, per loaded
/// extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct EngineKey {
    /// The host's identity.
    pub host: u64,
    /// The loaded extension's number ([`JsExtension::id`]).
    pub extension: u64,
}

/// One engine instance: a QuickJS runtime with one context, the extension's
/// modules loaded, the prelude evaluated, the sandbox installed, the limits
/// set.
pub(crate) struct Engine {
    // Declared first, so dropped first: a persistent value is freed while its
    // runtime is alive.
    /// The namespace object of `flm:_entry`, restored into every call.
    prelude: Persistent<Object<'static>>,
    /// The promises rejected with no handler since the last drain.
    rejections: Rejections,
    /// Frees the error objects of the refusals the module state holds.
    _refusals: FreeRefusals,
    context: Context,
    #[allow(dead_code)] // kept for the instance's life; the context holds it too
    runtime: Runtime,
    /// The state the module resolver and loader share: the refusals are read
    /// from it after the queued jobs run.
    modules: SharedState,
    /// The extension's stem, for the messages.
    stem: String,
    /// The deadline the interrupt handler compares the clock with; `None` while
    /// no handler runs or the budget is unlimited.
    deadline: Rc<Cell<Option<Instant>>>,
    /// Set by the interrupt handler when it fired; taken by [`Engine::fail`] to
    /// tell an interrupt from an ordinary exception.
    interrupted: Rc<Cell<bool>>,
    /// Set when the instance must not be used again (an interrupt, out of
    /// memory); the outermost call discards it on its way out.
    poisoned: Cell<bool>,
    /// The features whose `register` this instance has run (for real or
    /// rehearsed), by package and feature, with the configuration it ran with.
    pub registered: Registered,
    /// The live context, `ctx.as_raw()` of the outermost entry's
    /// `Context::with` scope: set when that scope opens, cleared by a guard
    /// before it closes (also when it unwinds); read by a nested entry
    /// ([`with_engine`] at a depth above 0) to reenter the context.
    live: Cell<Option<NonNull<rquickjs::qjs::JSContext>>>,
    /// How many entries into this instance are running on this thread.
    depth: Cell<u32>,
    /// The time budget of one handler call.
    time_budget: Option<Duration>,
    /// The memory limit, for the rendering of its failure.
    memory_limit: Option<usize>,
    /// Set by the instance's allocator when it refused an allocation (the
    /// memory limit reached); cleared at the start of each handler call.
    memory_reached: Rc<Cell<bool>>,
}

/// An engine instance [`Engine::create`] made, with what the files mode
/// hands back to the load.
pub(crate) struct Created {
    /// The instance.
    pub engine: Engine,
    /// The answer of the default export's call (the JSON text of the package
    /// objects), when the extension defines its packages.
    pub defined: Option<String>,
    /// The extension's files, taken back at the closing (files mode).
    pub files: Option<Box<dyn ExtensionFiles>>,
    /// The extension's modules the instance read or was given.
    pub recorded: BTreeMap<String, Arc<str>>,
}

/// Why an engine instance could not be created: one text per problem (the
/// refusals of the module resolver, or the one failure met), and the data
/// of the failure.
pub(crate) struct NotCreated {
    /// The texts.
    pub details: Vec<String>,
    /// The failure's data (an error's name and stack, a limit).
    pub data: serde_json::Value,
}

impl NotCreated {
    /// One text, no data.
    fn one(detail: String) -> NotCreated {
        NotCreated { details: vec![detail], data: json!(null) }
    }
}

/// What runs when a failure is rendered: an instance's creation (the
/// extension's stem), or a handler call (what the messages call it).
#[derive(Clone, Copy)]
enum Running<'a> {
    /// The creation of an instance of the extension of this stem.
    Creating(&'a str),
    /// A handler call, as the messages name it.
    Handler(&'a str),
}

impl Running<'_> {
    /// What runs, as a message's subject: "the modules of the extension
    /// `S`", or the handler call's name.
    fn subject(self) -> String {
        match self {
            Running::Creating(stem) => format!("the modules of the extension `{stem}`"),
            Running::Handler(what) => what.to_string(),
        }
    }
}

/// The deadline an engine's interrupt handler compares the clock with; `None`
/// while no budget of that engine runs or the budget is unlimited.
type DeadlineCell = Rc<Cell<Option<Instant>>>;

/// A running budget: the deadline cell of the call (or of the engine's
/// creation) it belongs to, and the budget of the call it interrupts, paused
/// until this one ends. See [`Engine::start_budget`]. Dropping it ends the
/// budget (also when the call unwinds).
pub(crate) struct PausedBudget {
    /// The deadline cell of the running call's engine.
    own: DeadlineCell,
    /// The deadline cell of the call this one interrupts, with the deadline it
    /// had; `None` when no other call was running on this thread.
    outer: Option<(DeadlineCell, Option<Instant>)>,
    /// When the outer budget was paused.
    paused_at: Instant,
}

impl PausedBudget {
    /// Starts the budget `budget` in the deadline cell `own`, pausing the
    /// budget of the innermost call running on this thread, whatever its
    /// engine.
    fn start(own: &DeadlineCell, budget: Option<Duration>) -> PausedBudget {
        let now = Instant::now();
        let outer = RUNNING.with(|running| {
            running.borrow().last().map(|cell| {
                let previous = cell.replace(None);
                (Rc::clone(cell), previous)
            })
        });
        own.set(deadline_after(now, budget));
        RUNNING.with(|running| running.borrow_mut().push(Rc::clone(own)));
        PausedBudget { own: Rc::clone(own), outer, paused_at: now }
    }
}

impl Drop for PausedBudget {
    fn drop(&mut self) {
        RUNNING.with(|running| {
            let mut running = running.borrow_mut();
            if running.last().is_some_and(|cell| Rc::ptr_eq(cell, &self.own)) {
                running.pop();
            }
        });
        // Cleared first: when the outer call runs on the same engine, the cell
        // is the same, and the outer's deadline is what it must hold after.
        self.own.set(None);
        if let Some((cell, previous)) = &self.outer {
            let elapsed = self.paused_at.elapsed();
            cell.set(previous.and_then(|deadline| deadline.checked_add(elapsed)));
        }
    }
}

/// The deadline `budget` after `now`: `None` when the budget is unlimited, or
/// so large that the deadline is past what the clock can represent.
fn deadline_after(now: Instant, budget: Option<Duration>) -> Option<Instant> {
    budget.and_then(|budget| now.checked_add(budget))
}

/// A duration in whole milliseconds, for a message (saturated at `u64::MAX`).
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

thread_local! {
    /// The deadline cells of the calls running on this thread (and of an
    /// engine's creation, whose modules have a call's budget), innermost
    /// last. Only the innermost budget runs; the others are paused.
    static RUNNING: RefCell<Vec<DeadlineCell>> = const { RefCell::new(Vec::new()) };
    /// The engines of this thread. Each is shared (`Rc`) between the map and
    /// the entries running on it, so the map is borrowed only for a moment and
    /// a nested entry into another engine can create it.
    static ENGINES: RefCell<HashMap<EngineKey, Kept>> = RefCell::new(HashMap::new());
}

/// An engine instance in a thread's map, with the loaded extension it is an
/// instance of, held weakly: once every holder of the extension is gone (the
/// registry, the factories, the handlers of a preparation that was replaced),
/// the next entry into any instance on the thread drops it ([`with_engine`]).
struct Kept {
    /// The instance.
    engine: Rc<Engine>,
    /// The loaded extension.
    extension: Weak<JsExtension>,
}

impl Engine {
    /// Creates an engine instance of `extension` on this thread: the runtime
    /// with its limits and the module resolver and loader, the context, the
    /// glue, the sandbox; then `flm:_entry` declared (which resolves and loads
    /// every module of the static import graph), the resolver closed, the
    /// modules evaluated, the queued jobs run; then, when the extension
    /// defines its packages, its default export called through
    /// `define(stem, main)` and the queued jobs run again.
    ///
    /// `files` is the files mode (the extension's files; `extension.modules`
    /// then holds the main module alone); `None` is the recorded mode.
    ///
    /// # Errors
    ///
    /// [`NotCreated`] with the refusals of the module resolver, or the one
    /// failure met (a module that does not parse or fails to evaluate, a
    /// top-level `await`, a limit, a default export that is not a function or
    /// fails).
    pub(crate) fn create(
        host: &JsHost,
        extension: &JsExtension,
        files: Option<Box<dyn ExtensionFiles>>,
    ) -> Result<Created, NotCreated> {
        // The extension's settings. The stack is decided by the thread this
        // instance is created on, and kept for every later call on it: a
        // worker thread has at least `WORKER_STACK_SIZE` (which bounds
        // `stack_mib`), any other thread what the host's setting states.
        let mut settings = extension.settings.clone();
        if !flm_core::lang::guard::is_worker_thread() {
            settings.stack_size = settings.stack_size.min(host.settings().stack_size);
        }
        let settings = &settings;
        let stem = extension.extension.stem.as_str();
        let deadline: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
        // Creating an instance has the budget of one handler call; a call
        // running on this thread (a creation inside a live call) is paused
        // for the whole creation.
        let budget = PausedBudget::start(&deadline, settings.time_budget);
        let failed = |detail: String| {
            NotCreated::one(format!("the engine of the extension `{stem}` could not start: {detail}"))
        };
        // The memory limit is the allocator's, which marks the instance when
        // it refuses (`memory.rs`); QuickJS's own limit is left unset.
        let (allocator, memory_reached) = Limited::new(settings.memory_limit);
        let runtime = Runtime::new_with_alloc(allocator).map_err(|e| failed(e.to_string()))?;
        runtime.set_max_stack_size(settings.stack_size);
        let interrupted = Rc::new(Cell::new(false));
        runtime.set_interrupt_handler(Some(Box::new({
            let deadline = Rc::clone(&deadline);
            let interrupted = Rc::clone(&interrupted);
            move || match deadline.get() {
                Some(at) if Instant::now() >= at => {
                    interrupted.set(true);
                    true
                }
                _ => false,
            }
        })));
        let state = ModuleState::shared(
            stem,
            &extension.main,
            host.module_sources().clone(),
            extension.modules.clone(),
            files,
        );
        runtime.set_loader(ModuleResolver(Rc::clone(&state)), ModuleLoader(Rc::clone(&state)));
        // Declared after the runtime, so dropped before it on a failure.
        let free_refusals = FreeRefusals(Rc::clone(&state));
        let rejections: Rejections = Rc::default();
        let tracker: rquickjs::runtime::RejectionTracker = Box::new({
            let rejections = Rc::clone(&rejections);
            let state = Rc::clone(&state);
            let memory_reached = Rc::clone(&memory_reached);
            move |ctx, promise, reason, handled| {
                track_rejection(&ctx, &rejections, &state, &memory_reached, promise, reason, handled)
            }
        });
        runtime.set_host_promise_rejection_tracker(Some(tracker));
        let context = Context::full(&runtime).map_err(|e| failed(e.to_string()))?;
        // The budget counts from here: what precedes is the binding's work.
        deadline.set(deadline_after(Instant::now(), settings.time_budget));
        let limits = Limits {
            interrupted: &interrupted,
            time_budget: settings.time_budget,
            memory_limit: settings.memory_limit,
            memory_reached: &memory_reached,
        };
        let mut files_back = None;
        let made = context.with(|ctx| {
            let creating = Running::Creating(stem);
            let fail = |error: rquickjs::Error, prefix: &str| {
                let caught = Err::<(), _>(error).catch(&ctx).err();
                let (error, limit) = render_failure(&ctx, caught, &limits, creating);
                let (detail, data) = match error {
                    ExtError::Handler { detail, data } => (detail, data),
                    other => (other.to_string(), json!(null)),
                };
                let detail = if limit || prefix.is_empty() { detail } else { format!("{prefix}{detail}") };
                NotCreated { details: vec![detail], data }
            };
            let invalid = format!("the modules of the extension `{stem}` must be valid JavaScript: ");
            let failing = format!("the modules of the extension `{stem}` must evaluate without an error: ");
            glue::install(&ctx, host, &extension.extension, extension.id).and_then(|()| ctx.eval::<(), _>(SANDBOX_JS)).map_err(|e| fail(e, ""))?;
            let declared = Module::declare(ctx.clone(), ENTRY, ENTRY_JS);
            let refusals = {
                let mut state = state.borrow_mut();
                files_back = state.close();
                state.take_refusals()
            };
            let module = match declared {
                Ok(_) | Err(_) if !refusals.is_empty() => {
                    let data = json!({ "refusals": refusals });
                    return Err(NotCreated { details: refusals, data });
                }
                Ok(module) => module,
                Err(error) => {
                    // QuickJS's message lacks the place, which the stack's
                    // first line gives (`at ./main.js:1:14`).
                    let mut not = fail(error, &invalid);
                    let place = not.data["stack"].as_str().and_then(|stack| {
                        stack.lines().map(str::trim).find_map(|line| line.strip_prefix("at "))
                    });
                    if let (Some(place), Some(detail)) = (place, not.details.first_mut()) {
                        let place = place.strip_prefix(super::modules::EXTENSION_PREFIX).unwrap_or(place);
                        detail.push_str(&format!(" (at {place})"));
                    }
                    return Err(not);
                }
            };
            let module = match module.eval().and_then(|(module, promise)| promise.finish::<()>().map(|()| module)) {
                Ok(module) => module,
                Err(rquickjs::Error::WouldBlock) => {
                    return Err(NotCreated::one(format!(
                        "the modules of the extension `{stem}` must finish evaluating without waiting: a top-level \
                         `await` is not supported"
                    )));
                }
                Err(error) => return Err(fail(error, &failing)),
            };
            drain_jobs(&ctx, &state, &rejections, &limits, creating)
                .map_err(|(details, data)| NotCreated { details, data })?;
            let namespace = module.namespace().map_err(|e| fail(e, &failing))?;
            let defined = if extension.define {
                let main = extension.main.as_str();
                namespace
                    .get::<_, Function>("define")
                    .and_then(|define| define.call::<_, ()>((stem, main)))
                    .map_err(|error| {
                        let mut not = fail(error, "");
                        let is_main = not.data["flm_error"]["kind"] == "main";
                        let limit = not.data.get("limit").is_some();
                        if !is_main && !limit {
                            for detail in &mut not.details {
                                *detail = format!(
                                    "the default export of the main module `{main}` of the extension `{stem}` must \
                                     return without an error: {detail}"
                                );
                            }
                        }
                        not
                    })?;
                // The default export's promise callbacks run before its
                // packages are read: an `async` default export declares them
                // all.
                let exporting = format!("the default export of the main module `{main}` of the extension `{stem}`");
                drain_jobs(&ctx, &state, &rejections, &limits, Running::Handler(&exporting))
                    .map_err(|(details, data)| NotCreated { details, data })?;
                let answer = namespace
                    .get::<_, Function>("packages")
                    .and_then(|packages| packages.call::<_, String>(()))
                    .map_err(|error| fail(error, ""))?;
                Some(answer)
            } else {
                None
            };
            Ok((Persistent::save(&ctx, namespace), defined))
        });
        drop(budget);
        let (prelude, defined) = made?;
        let recorded = state.borrow().recorded().clone();
        Ok(Created {
            engine: Engine {
                prelude,
                rejections,
                _refusals: free_refusals,
                context,
                runtime,
                modules: state,
                stem: stem.to_string(),
                deadline,
                interrupted,
                poisoned: Cell::new(false),
                registered: Registered::default(),
                live: Cell::new(None),
                depth: Cell::new(0),
                time_budget: settings.time_budget,
                memory_limit: settings.memory_limit,
                memory_reached,
            },
            defined,
            files: files_back,
            recorded,
        })
    }

    /// The namespace object of `flm:_entry`, in the context `ctx`.
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`] when the instance cannot restore it (it cannot
    /// allocate, or `ctx` is not this instance's).
    pub(crate) fn prelude<'js>(&self, ctx: &Ctx<'js>) -> Result<Object<'js>, ExtError> {
        self.prelude.clone().restore(ctx).map_err(|error| ExtError::Handler {
            detail: format!("the entry module of the engine of the extension `{}` could not be reached: {error}", self.stem),
            data: serde_json::Value::Null,
        })
    }

    /// Starts the time budget of one handler call; answers what
    /// [`Engine::end_budget`] needs to resume the budget of the call it
    /// interrupts, if any. A nested call has a budget of its own; the budget
    /// of the innermost call running on this thread, on this instance or on
    /// another, is paused meanwhile and resumes after. The outermost entry
    /// into the instance clears the memory limit's mark: an allocation
    /// refused during an earlier call, whose failure the script caught, is
    /// not this call's cause. A nested call (a template module's `describe`
    /// inside `register`, a handler a native runs) leaves it as it is: an
    /// allocation the outer call saw refused stays its cause after the nested
    /// call returns.
    pub(crate) fn start_budget(&self) -> PausedBudget {
        if self.depth.get() <= 1 {
            self.memory_reached.set(false);
        }
        PausedBudget::start(&self.deadline, self.time_budget)
    }

    /// Ends the time budget [`Engine::start_budget`] started, and resumes the
    /// outer call's, moved later by the time the inner call took. (Dropping
    /// the [`PausedBudget`] does the same.)
    pub(crate) fn end_budget(&self, paused: PausedBudget) {
        drop(paused);
    }

    /// Renders the failure `error` of `what` (a handler call on this
    /// instance) into [`ExtError::Handler`], and poisons the instance when
    /// the failure was an interrupt or an exhaustion of memory.
    pub(crate) fn fail(&self, ctx: &Ctx<'_>, error: rquickjs::Error, what: &str) -> ExtError {
        let caught = Err::<(), _>(error).catch(ctx).err();
        let limits = self.limits();
        let (error, poison) = render_failure(ctx, caught, &limits, Running::Handler(what));
        if poison {
            self.poisoned.set(true);
        }
        error
    }

    /// At the end of the outermost entry into this instance (`what`, a
    /// handler call): runs the queued promise jobs, then takes the refusals
    /// the module resolver recorded and the promises rejected with no handler
    /// meanwhile. Inside a nested entry it does nothing: the jobs queued by
    /// the call the nested one interrupts run when that call ends, while its
    /// objects are valid.
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`]: the refusals and the rejections, joined (each a
    /// refused `import()` or a promise rejected with no handler), or the time
    /// budget, exceeded by a job.
    pub(crate) fn drain(&self, ctx: &Ctx<'_>, what: &str) -> Result<(), ExtError> {
        self.drain_texts(ctx, what).map_err(|(details, data)| ExtError::Handler { detail: details.join("; "), data })
    }

    /// [`Engine::drain`], answering the texts one by one with the failure's
    /// data; poisons the instance when a job met a limit.
    fn drain_texts(&self, ctx: &Ctx<'_>, what: &str) -> Result<(), (Vec<String>, serde_json::Value)> {
        if self.depth.get() > 1 {
            return Ok(());
        }
        drain_jobs(ctx, &self.modules, &self.rejections, &self.limits(), Running::Handler(what)).inspect_err(
            |(_, data)| {
                if data.get("limit").is_some() {
                    self.poisoned.set(true);
                }
            },
        )
    }

    /// Ends the outermost entry into this instance whose call `what` answered
    /// `result`: every outermost entry ends with [`Engine::drain`], so the
    /// jobs a call queued run while its objects are valid and a failure
    /// inside them is that call's. A failure of any kind is followed by the
    /// texts of the drain (the refusals, the rejected promises, or a limit
    /// met by a job): [`ExtError::Followed`] when the drain met something,
    /// the failure as it is otherwise. On an instance a limit poisoned, the
    /// jobs do not run (the instance is discarded); the refusals and the
    /// rejected promises recorded during the call follow the failure. Inside
    /// a nested entry, `result` as it is.
    ///
    /// # Errors
    ///
    /// `result`'s error (followed by the drain's texts), or
    /// [`Engine::drain`]'s.
    pub(crate) fn finish<T>(&self, ctx: &Ctx<'_>, what: &str, result: Result<T, ExtError>) -> Result<T, ExtError> {
        let error = match result {
            Ok(value) => return self.drain(ctx, what).map(|()| value),
            Err(error) if self.depth.get() > 1 => return Err(error),
            Err(error) => error,
        };
        let texts = if self.poisoned.get() {
            let mut texts = self.modules.borrow_mut().take_refusals();
            texts.extend(take_rejections(&self.rejections, Running::Handler(what)));
            texts
        } else {
            self.drain_texts(ctx, what).err().map(|(texts, _)| texts).unwrap_or_default()
        };
        if texts.is_empty() {
            return Err(error);
        }
        Err(match error {
            ExtError::Followed { first, mut then } => {
                then.extend(texts);
                ExtError::Followed { first, then }
            }
            first => ExtError::Followed { first: Box::new(first), then: texts },
        })
    }

    /// The limits, for the rendering of a failure.
    fn limits(&self) -> Limits<'_> {
        Limits {
            interrupted: &self.interrupted,
            time_budget: self.time_budget,
            memory_limit: self.memory_limit,
            memory_reached: &self.memory_reached,
        }
    }
}

/// What the rendering of a failure needs to tell a limit from an ordinary
/// exception.
struct Limits<'a> {
    /// Set by the interrupt handler when it fired.
    interrupted: &'a Cell<bool>,
    /// The time budget of one call.
    time_budget: Option<Duration>,
    /// The memory limit.
    memory_limit: Option<usize>,
    /// Set by the instance's allocator when it refused an allocation.
    memory_reached: &'a Cell<bool>,
}

/// The text of an exceeded time budget, for `running`.
fn time_text(running: Running<'_>, budget_ms: u64) -> String {
    match running {
        Running::Creating(stem) => {
            format!("the modules of the extension `{stem}` must finish evaluating within the time budget of {budget_ms} ms")
        }
        Running::Handler(what) => format!("{what} must finish within its time budget of {budget_ms} ms"),
    }
}

/// The text and the data of the memory limit `limit` reached by `running`.
fn memory_failure(running: Running<'_>, limit: Option<usize>) -> (String, serde_json::Value) {
    let limit = limit.unwrap_or(0);
    let detail = match running {
        Running::Creating(stem) => {
            format!("the modules of the extension `{stem}` must evaluate within the memory limit of {limit} bytes")
        }
        Running::Handler(what) => format!("{what} must stay within the memory limit of {limit} bytes"),
    };
    (detail, json!({ "limit": "memory", "limit_bytes": limit }))
}

/// Runs the queued jobs until none is left; answers the texts of the
/// refusals recorded meanwhile and of the promises still rejected with no
/// handler, or the time budget's text when a job was interrupted, with the
/// failure's data. A promise rejected because a job reached the memory limit
/// answers the limit's text first and its data, as a handler's own call
/// reaching it does ([`render_failure`]), followed by the other texts.
fn drain_jobs(
    ctx: &Ctx<'_>,
    state: &SharedState,
    rejections: &Rejections,
    limits: &Limits<'_>,
    running: Running<'_>,
) -> Result<(), (Vec<String>, serde_json::Value)> {
    while ctx.execute_pending_job() {}
    if limits.interrupted.replace(false) {
        let budget_ms = limits.time_budget.map_or(0, millis);
        return Err((vec![time_text(running, budget_ms)], json!({ "limit": "time", "budget_ms": budget_ms })));
    }
    let refusals = state.borrow_mut().take_refusals();
    let (memory, rejected) = take_rejections_apart(rejections, running);
    if memory {
        let (detail, data) = memory_failure(running, limits.memory_limit);
        return Err((std::iter::once(detail).chain(refusals).chain(rejected).collect(), data));
    }
    if refusals.is_empty() && rejected.is_empty() {
        return Ok(());
    }
    let mut data = serde_json::Map::new();
    if !refusals.is_empty() {
        data.insert("refusals".to_string(), json!(refusals));
    }
    if !rejected.is_empty() {
        data.insert("rejections".to_string(), json!(rejected));
    }
    Err((refusals.into_iter().chain(rejected).collect(), serde_json::Value::Object(data)))
}

/// The texts of the promises rejected with no handler that `rejections`
/// holds, worded for `running`; the promises are freed.
fn take_rejections(rejections: &Rejections, running: Running<'_>) -> Vec<String> {
    let taken = std::mem::take(&mut *rejections.borrow_mut());
    let subject = running.subject();
    taken
        .into_iter()
        .map(|(_, reason, _)| format!("{subject} must leave no promise rejected without a handler: {reason}"))
        .collect()
}

/// [`take_rejections`], with the promises rejected because the memory limit
/// was reached set apart: answers whether there was one, and the texts of
/// the others.
fn take_rejections_apart(rejections: &Rejections, running: Running<'_>) -> (bool, Vec<String>) {
    let taken = std::mem::take(&mut *rejections.borrow_mut());
    let subject = running.subject();
    let memory = taken.iter().any(|(_, _, memory)| *memory);
    let texts = taken
        .into_iter()
        .filter(|(_, _, memory)| !memory)
        .map(|(_, reason, _)| format!("{subject} must leave no promise rejected without a handler: {reason}"))
        .collect();
    (memory, texts)
}

/// The runtime's promise rejection tracker: records `promise`, rejected with
/// no handler, with the text of `reason` (an `Error`'s message, any other
/// value by `String()`) and whether `reason` is the memory limit reached
/// (QuickJS's out-of-memory error or `null`, after the instance's allocator
/// refused an allocation, as [`render_failure`] reads a failure); forgets it when a handler is
/// attached later (`handled`). A rejection whose reason is the error object
/// of a refusal of the module resolver (a refused `import()`), the same
/// object, is not recorded: the refusal is.
fn track_rejection<'js>(
    ctx: &Ctx<'js>,
    rejections: &Rejections,
    state: &SharedState,
    memory_reached: &Cell<bool>,
    promise: Value<'js>,
    reason: Value<'js>,
    handled: bool,
) {
    if handled {
        rejections.borrow_mut().retain(|(kept, _, _)| kept.clone().restore(ctx).map_or(true, |kept| kept != promise));
        return;
    }
    if state.borrow().is_refusal(ctx, &reason) {
        return;
    }
    let memory = memory_reached.get()
        && (reason.is_null()
            || reason
                .as_object()
                .and_then(|object| Exception::from_object(object.clone()))
                .is_some_and(|exception| exception.message().as_deref() == Some(OUT_OF_MEMORY_MESSAGE)));
    // Rendered before the list is borrowed: `String()` may run the script's
    // code, which may reject another promise.
    let text = match reason.as_object().and_then(|object| Exception::from_object(object.clone())) {
        Some(exception) => exception.message().unwrap_or_default(),
        None => reason.get::<Coerced<String>>().map(|text| text.0).unwrap_or_default(),
    };
    let text = if text.is_empty() { reason_fallback(ctx, reason) } else { text };
    rejections.borrow_mut().push((Persistent::save(ctx, promise), text, memory));
}

/// The text of a rejection's `reason` whose message or `String()` form is
/// empty or cannot be read: its JSON form, else its `String()` form for an
/// `Error` with an empty message (its name), else a fixed wording.
fn reason_fallback<'js>(ctx: &Ctx<'js>, reason: Value<'js>) -> String {
    let json = ctx.json_stringify(reason.clone()).ok().flatten().and_then(|text| text.to_string().ok());
    if let Some(json) = json.filter(|json| !json.is_empty() && json != "{}") {
        return json;
    }
    match reason.get::<Coerced<String>>() {
        Ok(text) if !text.0.is_empty() => text.0,
        _ => "a value without a text form".to_string(),
    }
}

/// Renders a caught failure of `running`, telling a limit from an ordinary
/// exception: answers the error and whether it was a limit (after which the
/// instance must be discarded).
fn render_failure<'js>(
    ctx: &Ctx<'js>,
    caught: Option<CaughtError<'js>>,
    limits: &Limits<'_>,
    running: Running<'_>,
) -> (ExtError, bool) {
    if limits.interrupted.replace(false) {
        let budget_ms = limits.time_budget.map_or(0, millis);
        return (
            ExtError::Handler {
                detail: time_text(running, budget_ms),
                data: json!({ "limit": "time", "budget_ms": budget_ms }),
            },
            true,
        );
    }
    let Some(caught) = caught else {
        let subject = running.subject();
        return (
            ExtError::Handler { detail: format!("a failure of {subject} must leave an exception"), data: json!(null) },
            false,
        );
    };
    if limits.memory_reached.get() && (is_out_of_memory(&caught) || is_null(&caught)) {
        let (detail, data) = memory_failure(running, limits.memory_limit);
        return (ExtError::Handler { detail, data }, true);
    }
    (render_caught(ctx, caught), false)
}

/// Whether `caught` is a thrown `null`: what QuickJS throws when it runs out
/// of memory so close to the limit that it cannot allocate its
/// `InternalError` either (`quickjs.c:8446` in rquickjs-sys 0.14.0, "out of
/// memory: throw JS_NULL to avoid recursing"). It is read as the limit only
/// when the instance's allocator refused an allocation during the call
/// ([`memory`](super::memory)); a script's own `throw null` is an ordinary
/// exception.
fn is_null(caught: &CaughtError<'_>) -> bool {
    matches!(caught, CaughtError::Value(value) if value.is_null())
}

/// Whether `caught` is QuickJS's exhaustion of memory.
fn is_out_of_memory(caught: &CaughtError<'_>) -> bool {
    match caught {
        CaughtError::Error(rquickjs::Error::Allocation) => true,
        CaughtError::Exception(exception) => exception.message().as_deref() == Some(OUT_OF_MEMORY_MESSAGE),
        _ => false,
    }
}

/// Renders a caught JavaScript failure into an [`ExtError`].
///
/// - A thrown `FlmError` of kind `name_outside_extension` (the name rule's
///   refusal, [`names::check`](crate::adapter::names::check)): that refusal,
///   [`ExtError::NameOutsideExtension`], its fields read back from the
///   error's data, so that it keeps its kind through the binding.
/// - An `Error` instance: `detail` its message; `data` its `name`, its `stack`,
///   and `flm_error`, the `data` of a thrown `FlmError` (else `null`).
/// - Any other thrown value: `detail` its `String()` form; `data` `{"value":
///   <its JSON, or null>}`.
/// - A failure of rquickjs itself: `detail` its text.
pub(crate) fn render_caught<'js>(ctx: &Ctx<'js>, caught: CaughtError<'js>) -> ExtError {
    match caught {
        CaughtError::Exception(exception) => {
            let name: Option<String> = exception.get("name").ok().flatten();
            let message = exception.message().unwrap_or_default();
            let flm_error = if name.as_deref() == Some("FlmError") {
                exception.get::<_, Value>("data").ok().and_then(|data| to_json(ctx, data)).unwrap_or(json!(null))
            } else {
                json!(null)
            };
            if let Some(refusal) = name_outside_extension(&flm_error) {
                return refusal;
            }
            ExtError::Handler {
                detail: message,
                data: json!({ "name": name, "stack": exception.stack(), "flm_error": flm_error }),
            }
        }
        CaughtError::Value(value) => {
            let detail = value.get::<Coerced<String>>().map(|text| text.0).unwrap_or_default();
            let json = to_json(ctx, value).unwrap_or(json!(null));
            ExtError::Handler { detail, data: json!({ "value": json }) }
        }
        CaughtError::Error(error) => ExtError::Handler { detail: error.to_string(), data: json!(null) },
    }
}

/// The name rule's refusal that `data`, the data of a thrown `FlmError`,
/// carries: [`ExtError::NameOutsideExtension`] when its kind is
/// `name_outside_extension` and its four fields are texts, as
/// [`ExtError::data`] writes them.
fn name_outside_extension(data: &serde_json::Value) -> Option<ExtError> {
    if data["kind"] != "name_outside_extension" {
        return None;
    }
    let field = |key: &str| data[key].as_str().map(str::to_string);
    Some(ExtError::NameOutsideExtension {
        what: field("what")?,
        name: field("name")?,
        package: field("package")?,
        extension: field("extension")?,
    })
}

/// A JavaScript value as JSON, when `JSON.stringify` answers for it.
fn to_json<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> Option<serde_json::Value> {
    let text = ctx.json_stringify(value).ok()??.to_string().ok()?;
    serde_json::from_str(&text).ok()
}

/// Leaves the engine on its way out of [`with_engine`]: lowers the depth, and
/// at the outermost level discards a poisoned engine, or any in strict mode.
struct Leave<'a> {
    key: EngineKey,
    engine: &'a Rc<Engine>,
    strict: bool,
}

impl Drop for Leave<'_> {
    fn drop(&mut self) {
        let depth = self.engine.depth.get().saturating_sub(1);
        self.engine.depth.set(depth);
        if depth == 0 && (self.strict || self.engine.poisoned.get()) {
            ENGINES.with(|engines| {
                let mut engines = engines.borrow_mut();
                if engines.get(&self.key).is_some_and(|kept| Rc::ptr_eq(&kept.engine, self.engine)) {
                    engines.remove(&self.key);
                }
            });
        }
    }
}

/// Runs `f` with the engine instance of `extension` on this thread, creating
/// it first in the recorded mode when the thread has none (or strict mode
/// asks for a fresh one). At a depth above 0 (a nested handler call), `f`
/// runs on the engine's live context, kept by the outermost entry, instead of
/// entering `Context::with` again.
///
/// Before the lookup, the thread's instances of extensions no one holds any
/// more (a loader's extensions after its registry, its factories and their
/// handlers are dropped) are dropped, unless a call is running on them.
/// After `f`, at the outermost level only: a poisoned instance, or any
/// instance in strict mode, is removed from the thread's map and dropped.
///
/// # Errors
///
/// [`ExtError::Handler`]: the instance cannot be created on this thread, or
/// a nested entry while the instance's outermost entry has no open context.
pub(crate) fn with_engine<R>(
    host: &JsHost,
    extension: &Arc<JsExtension>,
    f: impl FnOnce(&Ctx<'_>, &Engine) -> R,
) -> Result<R, ExtError> {
    let key = EngineKey { host: host.id(), extension: extension.id };
    let strict = host.settings().strict;
    let swept = ENGINES.with(|engines| {
        let mut engines = engines.borrow_mut();
        let mut swept = Vec::new();
        engines.retain(|_, kept| {
            let keep = kept.extension.strong_count() > 0 || kept.engine.depth.get() > 0;
            if !keep {
                swept.push(Rc::clone(&kept.engine));
            }
            keep
        });
        swept
    });
    // Dropped after the map's borrow ends.
    drop(swept);
    let existing = ENGINES.with(|engines| engines.borrow().get(&key).map(|kept| Rc::clone(&kept.engine)));
    let engine = match existing {
        Some(engine) if engine.depth.get() > 0 || !strict => engine,
        _ => {
            let created = Engine::create(host, extension, None).map_err(|not| ExtError::Handler {
                detail: format!(
                    "the engine of the extension `{}` could not be created on this thread: {}",
                    extension.extension.stem,
                    not.details.join("; ")
                ),
                data: not.data,
            })?;
            let engine = Rc::new(created.engine);
            let kept = Kept { engine: Rc::clone(&engine), extension: Arc::downgrade(extension) };
            ENGINES.with(|engines| engines.borrow_mut().insert(key, kept));
            engine
        }
    };
    let nested = engine.depth.get() > 0;
    let live = if nested {
        Some(engine.live.get().ok_or_else(|| ExtError::Handler {
            detail: format!(
                "a nested entry into the engine of the extension `{}` must come from a handler call running on it",
                extension.extension.stem
            ),
            data: serde_json::Value::Null,
        })?)
    } else {
        None
    };
    engine.depth.set(engine.depth.get() + 1);
    let _leave = Leave { key, engine: &engine, strict };
    Ok(match live {
        Some(ctx) => reenter(ctx, |ctx| f(&ctx, &engine)),
        None => engine.context.with(|ctx| {
            let _live = Live::open(&engine.live, &ctx);
            f(&ctx, &engine)
        }),
    })
}

/// The live context of an engine while its outermost entry's `Context::with`
/// scope is open: [`Live::open`] sets it, dropping the guard clears it (also
/// when the scope unwinds).
struct Live<'a>(&'a Cell<Option<NonNull<rquickjs::qjs::JSContext>>>);

impl<'a> Live<'a> {
    /// Sets `cell` to the context `ctx` until the guard drops.
    fn open(cell: &'a Cell<Option<NonNull<rquickjs::qjs::JSContext>>>, ctx: &Ctx<'_>) -> Live<'a> {
        cell.set(Some(ctx.as_raw()));
        Live(cell)
    }
}

impl Drop for Live<'_> {
    fn drop(&mut self) {
        self.0.set(None);
    }
}

/// Keeps `engine`, the instance the load of `extension` created, in this
/// thread's map, so that the registry build on the loading thread uses it;
/// the map holds `extension` weakly ([`with_engine`] drops the instance once
/// the extension is gone).
pub(crate) fn keep(host: &JsHost, extension: &Arc<JsExtension>, engine: Engine) {
    let key = EngineKey { host: host.id(), extension: extension.id };
    let kept = Kept { engine: Rc::new(engine), extension: Arc::downgrade(extension) };
    ENGINES.with(|engines| engines.borrow_mut().insert(key, kept));
}

/// Runs `f` on the live context `ctx` of an engine already entered on this
/// thread.
fn reenter<R>(ctx: NonNull<rquickjs::qjs::JSContext>, f: impl for<'js> FnOnce(Ctx<'js>) -> R) -> R {
    // SAFETY: `ctx` comes from `Engine::live`, which holds a pointer only
    // between the outermost entry's `Live::open`, inside its `Context::with`
    // scope, and the guard's drop before that scope ends (also when it
    // unwinds). A pointer read from it therefore names a context whose scope
    // is open (the runtime's lock is held by it), and that scope outlives this
    // call, since a nested entry returns before the entry it interrupts (a
    // native of that entry called it). `Ctx::from_raw` duplicates the context
    // and the `Ctx` frees its reference when dropped. The lifetime `'js` is
    // the fresh lifetime of `f`'s argument, so no value of this context
    // escapes `f`, as with `Context::with`.
    let ctx = unsafe { Ctx::from_raw(ctx) };
    f(ctx)
}

/// Drops every engine instance of the host `host` on this thread.
pub(crate) fn discard_host(host: u64) {
    let discarded: Vec<Kept> = ENGINES.with(|engines| {
        let mut engines = engines.borrow_mut();
        let keys: Vec<EngineKey> = engines.keys().filter(|key| key.host == host).copied().collect();
        keys.into_iter().filter_map(|key| engines.remove(&key)).collect()
    });
    drop(discarded);
}

/// How many engine instances of the host `host` this thread's map holds.
pub(crate) fn count_host(host: u64) -> usize {
    ENGINES.with(|engines| engines.borrow().keys().filter(|key| key.host == host).count())
}

/// An extension for the binding's tests: modules by path, the first the main
/// module, whose default export is never called; its identity and stem are
/// `test`. Not part of the interface.
#[doc(hidden)]
#[derive(Clone)]
pub struct TestPackage(Arc<JsExtension>);

impl TestPackage {
    /// An extension of `host` holding `modules`, each a path inside the
    /// extension (normalized) and a text; the first is the main module. The
    /// import rules are those of a loaded extension.
    ///
    /// # Errors
    ///
    /// [`ExtError::Argument`] for a path that is absolute or leaves the
    /// extension's folder, or for no module at all.
    pub fn new(host: &JsHost, modules: &[(&str, &str)]) -> Result<TestPackage, ExtError> {
        let refused = |detail: String| ExtError::Argument { method: "TestPackage::new".to_string(), detail };
        let mut recorded = BTreeMap::new();
        let mut main = None;
        for (path, text) in modules {
            let normalized = paths::normalize(path).map_err(|error| refused(error.to_string()))?;
            main.get_or_insert_with(|| normalized.clone());
            recorded.insert(normalized, Arc::from(*text));
        }
        let main = main.ok_or_else(|| refused("a test package must have a main module".to_string()))?;
        Ok(TestPackage(Arc::new(JsExtension {
            id: binding::next_extension_id(),
            host: host.clone(),
            settings: host.settings().clone(),
            extension: ExtensionIdentity { identity: "test".to_string(), stem: "test".to_string() },
            main,
            modules: recorded,
            define: false,
        })))
    }
}

/// Runs `source` as one handler call of `package` on this thread's engine
/// instance, and answers the `String()` form of its completion value. For
/// the binding's tests: no API module is involved. Not part of the
/// interface.
///
/// `source` is the body of a function whose one parameter `main` is the
/// namespace of the package's main module; it runs in a frame of its own
/// (with an empty handle table), with the call's time budget, after the
/// sandbox's random sequence is restarted, and the queued jobs run after it
/// as after any handler call. Inside it, the global function
/// `__flm_test_nested(source)` runs `source` the same way as a nested handler
/// call of the same package, and answers its result, or `error: <detail>`;
/// `__flm_test_nested_in(index, source)` does the same in the package
/// `index` of the list [`set_test_packages`] gave this thread. The messages
/// call the script "the handler".
///
/// # Errors
///
/// [`ExtError::Handler`]: the instance's creation failure, or the call's.
#[doc(hidden)]
pub fn eval_for_test(host: &JsHost, package: &TestPackage, source: &str) -> Result<String, ExtError> {
    const WHAT: &str = "the handler";
    let extension = &package.0;
    with_engine(host, extension, |ctx, engine| {
        let mut table = CallTable::new(frames::next_call());
        let registration = Registration {
            binding: Arc::clone(extension) as Arc<dyn binding::Binding>,
            package: String::new(),
            feature: String::new(),
            settings: Arc::new(serde_json::Value::Null),
        };
        let frame = Frame { call: table.call(), registration, table: &mut table };
        frames::enter(frame, || {
            let previous = engine.start_budget();
            let result = engine.prelude(ctx).and_then(|namespace| {
                run_test_source(ctx, &namespace, extension, source).map_err(|error| engine.fail(ctx, error, WHAT))
            });
            let result = engine.finish(ctx, WHAT, result);
            engine.end_budget(previous);
            result
        })
    })?
}

/// The body of [`eval_for_test`] inside its frame, on an engine instance of
/// `extension`.
fn run_test_source<'js>(
    ctx: &Ctx<'js>,
    namespace: &Object<'js>,
    extension: &Arc<JsExtension>,
    source: &str,
) -> rquickjs::Result<String> {
    let globals = ctx.globals();
    if !globals.contains_key("__flm_test_nested")? {
        // Held weakly: the instance must not keep its extension alive.
        let extension = Arc::downgrade(extension);
        let nested = move |source: String| test_nested(&extension, source);
        globals.set("__flm_test_nested", Function::new(ctx.clone(), nested)?)?;
        globals.set("__flm_test_nested_in", Function::new(ctx.clone(), test_nested_in)?)?;
    }
    if let Ok(begin) = globals.get::<_, Function>("__flm_begin_call") {
        begin.call::<_, ()>(())?;
    }
    let main: Object = namespace.get("MAIN")?;
    let function: Function = ctx.eval(format!("(function (main) {{\n{source}\n}})"))?;
    let value: Coerced<String> = function.call((main,))?;
    Ok(value.0)
}

/// `__flm_test_nested(source)`: [`eval_for_test`] of the running call's
/// package, from within a native. The running call is the innermost frame,
/// which runs on the engine instance whose script calls the native: an
/// instance of `extension`.
fn test_nested(extension: &Weak<JsExtension>, source: String) -> String {
    let running = |extension: &Arc<JsExtension>| frames::with_current(extension.id, |_| ()).is_some();
    let Some(extension) = extension.upgrade().filter(running) else {
        return "error: no handler call is running".to_string();
    };
    let host = extension.host.clone();
    match eval_for_test(&host, &TestPackage(extension), &source) {
        Ok(text) => text,
        Err(error) => format!("error: {error}"),
    }
}

thread_local! {
    /// The packages `__flm_test_nested_in` reaches, by index.
    static TEST_PACKAGES: RefCell<Vec<TestPackage>> = const { RefCell::new(Vec::new()) };
}

/// Gives this thread the packages `__flm_test_nested_in(index, source)`
/// reaches (see [`eval_for_test`]). For the binding's tests. Not part of the
/// interface.
#[doc(hidden)]
pub fn set_test_packages(packages: &[TestPackage]) {
    TEST_PACKAGES.with(|list| *list.borrow_mut() = packages.to_vec());
}

/// `__flm_test_nested_in(index, source)`: [`eval_for_test`] of the package
/// `index` of [`set_test_packages`]'s list, from within a native.
fn test_nested_in(index: usize, source: String) -> String {
    let Some(package) = TEST_PACKAGES.with(|list| list.borrow().get(index).cloned()) else {
        return format!("error: the test package {index} must have been given");
    };
    let host = package.0.host.clone();
    match eval_for_test(&host, &package, &source) {
        Ok(text) => text,
        Err(error) => format!("error: {error}"),
    }
}
