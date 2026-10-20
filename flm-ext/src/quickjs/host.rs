//! The engine host: the settings every engine is created with, the modules the
//! program offers to every extension, and where `console` output goes.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::host::ConsoleSink;


/// The settings every engine of a host is created with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsSettings {
    /// The memory an engine may allocate, in bytes; `None` is unlimited.
    /// Default: 64 MiB. Exceeding it fails the running handler and discards the
    /// engine.
    pub memory_limit: Option<usize>,
    /// The time one handler call may run; `None` is unlimited. Default: 10 s.
    /// Exceeding it interrupts the handler, fails it, and discards the engine.
    /// A nested handler call has a budget of its own; the outer call's resumes.
    /// Creating an engine instance (evaluating the extension's modules and
    /// calling its default export) has the same budget.
    pub time_budget: Option<Duration>,
    /// The JavaScript stack an engine may use, in bytes. Default: 1 MiB. It must
    /// stay below what remains of the real stack of every thread that runs a
    /// handler: `flm-core`'s workers have 64 MiB; a thread the program creates
    /// has what the program gave it (2 MiB by default from Rust). Exceeding it
    /// is an ordinary exception the handler may catch (`RangeError`); the engine
    /// is kept.
    ///
    /// The host's value is also what the program states its threads that are
    /// not worker threads allow. An engine instance keeps the stack it was
    /// created with for every call made on it, so the stack is decided by the
    /// thread the instance is created on: an instance created on a worker
    /// thread (`flm_core::lang::guard::is_worker_thread`) gets the extension's
    /// `stack_mib`; one created on any other thread, such as the thread that
    /// loads the extension, gets the smaller of `stack_mib` and this value.
    /// The value is not capped: a host that sets it must keep it below the
    /// real stack of every thread that creates an instance (below the worker
    /// threads' `WORKER_STACK_SIZE` on a worker thread), or a deep recursion
    /// overflows the thread instead of raising a `RangeError`.
    pub stack_size: usize,
    /// Strict mode: a fresh engine for every handler call, so that nothing a
    /// script keeps at module level survives a call. For tests. Default: off.
    pub strict: bool,
}

impl JsSettings {
    /// These settings with the limits an extension declares in its manifest
    /// (`extension.js.limits`): each limit given replaces the matching
    /// setting (`memory_mib` the memory, `stack_mib` the stack, `time_s` the
    /// time budget, each converted from MiB or seconds); an absent one keeps
    /// it. What every engine instance of that extension is created with, but
    /// for the stack of an instance created on a thread that is not a worker
    /// thread, which is at most the host's value (see
    /// [`stack_size`](JsSettings::stack_size)).
    pub fn with_limits(&self, limits: &crate::manifest::JsLimits) -> JsSettings {
        let mib = |n: u64| usize::try_from(n).ok().and_then(|n| n.checked_mul(1 << 20)).unwrap_or(usize::MAX);
        let mut settings = self.clone();
        if let Some(memory) = limits.memory_mib {
            settings.memory_limit = Some(mib(memory));
        }
        if let Some(stack) = limits.stack_mib {
            settings.stack_size = mib(stack);
        }
        if let Some(time) = limits.time_s {
            settings.time_budget = Some(Duration::from_secs(time));
        }
        settings
    }
}

impl Default for JsSettings {
    fn default() -> JsSettings {
        JsSettings {
            memory_limit: Some(64 << 20),
            time_budget: Some(Duration::from_secs(10)),
            stack_size: 1 << 20,
            strict: false,
        }
    }
}

/// The engine host: the settings, the modules the program offers to every
/// extension, the console sink. Cheap to clone (an `Arc` inside); every clone
/// is the same host, and the engines are keyed by it.
#[derive(Clone)]
pub struct JsHost(Arc<HostInner>);

struct HostInner {
    id: u64,
    settings: JsSettings,
    modules: BTreeMap<String, Arc<str>>,
    console: ConsoleSink,
}

/// The process-wide counter of hosts: two hosts never share engines, even with
/// equal settings.
static NEXT_HOST: AtomicU64 = AtomicU64::new(1);

impl JsHost {
    /// A host with `settings`, no modules, and `console` written to standard
    /// error as `[js <level>] <text>`.
    pub fn new(settings: JsSettings) -> JsHost {
        JsHost(Arc::new(HostInner {
            id: NEXT_HOST.fetch_add(1, Ordering::Relaxed),
            settings,
            modules: BTreeMap::new(),
            console: Arc::new(|level, text| eprintln!("[js {level}] {text}")),
        }))
    }

    /// Adds a module every extension loaded with this host may import by
    /// `name`, a bare name (`lodash`, `mathjax/core`). A host module imports
    /// another of the host's modules by its name, or by a path relative to
    /// its own name (`./util` from `mathjax/core` is `mathjax/util`).
    ///
    /// # Errors
    ///
    /// [`HostModuleError`] for a name that is empty, holds a NUL character,
    /// begins with `.`, `/` or a backslash (a path), holds `:` (which marks the
    /// extension API, `flm:`, and other schemes), or is already offered.
    pub fn with_module(self, name: &str, source: &str) -> Result<JsHost, HostModuleError> {
        let detail = if name.is_empty() {
            Some("the name of a module this program offers must not be empty".to_string())
        } else if name.contains('\0') {
            Some(format!("the module name `{}` this program offers must not hold a NUL character", name.escape_debug()))
        } else if name.starts_with('.') || name.starts_with('/') || name.starts_with('\\') {
            Some(format!("the module name `{name}` this program offers must be a bare name, not a path"))
        } else if name.contains(':') {
            Some(format!(
                "the module name `{name}` this program offers must not hold `:`, which marks the extension API \
                 (`flm:`) and other schemes"
            ))
        } else if self.0.modules.contains_key(name) {
            Some(format!("this program offers two modules named `{name}`"))
        } else {
            None
        };
        if let Some(detail) = detail {
            return Err(HostModuleError { name: name.to_string(), detail });
        }
        Ok(self.rebuild(|inner| {
            inner.modules.insert(name.to_string(), Arc::from(source));
        }))
    }

    /// Routes `console` output to `sink`.
    pub fn with_console(self, sink: ConsoleSink) -> JsHost {
        self.rebuild(|inner| inner.console = sink)
    }

    /// The settings every engine of this host is created with.
    pub fn settings(&self) -> &JsSettings {
        &self.0.settings
    }

    /// The host's modules, by name, in name order.
    pub fn modules(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.modules.iter().map(|(name, source)| (name.as_str(), &**source))
    }

    /// Drops every engine instance this host's extensions created on the
    /// calling thread.
    /// Tests use it between scenarios; a program need not call it.
    pub fn discard_engines_on_this_thread(&self) {
        super::engine::discard_host(self.id());
    }

    /// How many engine instances of this host's extensions the calling
    /// thread holds. For the binding's tests. Not part of the interface.
    #[doc(hidden)]
    pub fn engines_on_this_thread(&self) -> usize {
        super::engine::count_host(self.id())
    }

    /// The host's identity, which keys its engines.
    pub(crate) fn id(&self) -> u64 {
        self.0.id
    }

    /// The console sink.
    pub(crate) fn console(&self) -> &ConsoleSink {
        &self.0.console
    }

    /// The host's modules, with their shared sources.
    pub(crate) fn module_sources(&self) -> &BTreeMap<String, Arc<str>> {
        &self.0.modules
    }

    /// The host with `change` applied. When no other clone of this host
    /// exists, the identity is kept: an engine instance created with it may
    /// outlive the extensions that used it, but no extension of the changed
    /// host can reach that instance, since an instance's key holds its loaded
    /// extension's number, which is never given twice. Otherwise the result is a new host, with an
    /// identity of its own, so that the clone and the result never share
    /// engines.
    fn rebuild(self, change: impl FnOnce(&mut HostInner)) -> JsHost {
        let mut inner = match Arc::try_unwrap(self.0) {
            Ok(inner) => inner,
            Err(shared) => HostInner {
                id: NEXT_HOST.fetch_add(1, Ordering::Relaxed),
                settings: shared.settings.clone(),
                modules: shared.modules.clone(),
                console: Arc::clone(&shared.console),
            },
        };
        change(&mut inner);
        JsHost(Arc::new(inner))
    }
}

impl fmt::Debug for JsHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsHost")
            .field("id", &self.0.id)
            .field("settings", &self.0.settings)
            .field("modules", &self.0.modules.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

/// A module name [`JsHost::with_module`] refuses: the name and the rule it
/// breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostModuleError {
    /// The name, as given.
    pub name: String,
    /// The rule it breaks, as text for a person.
    pub detail: String,
}

impl fmt::Display for HostModuleError {
    /// The detail.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for HostModuleError {}
