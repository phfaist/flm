//! The engine host of the `wasm` binding: the settings every instance of a
//! module is created with, and where `console` output goes.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::host::ConsoleSink;
use crate::manifest::WasmLimits;

/// The settings every instance of a host's modules is created with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmSettings {
    /// The memory an instance may have, in bytes; `None` is unlimited.
    /// Default: 64 MiB. A module whose initial memory is above it cannot be
    /// instantiated; a growth past it is refused (`memory.grow` answers -1),
    /// and when the handler then traps, it fails with the memory limit's text
    /// and the instance is discarded.
    pub memory_limit: Option<usize>,
    /// The fuel one handler call may consume, in wasmi's units (about one per
    /// instruction); `None` is unlimited. Default: 1,000,000,000. Running out
    /// fails the handler and discards the instance. A nested handler call has
    /// fuel of its own; the outer call's remainder is restored when it
    /// returns. Creating an instance (instantiating the module and its entry
    /// `define`) has the same fuel.
    pub fuel: Option<u64>,
    /// Strict mode: a fresh instance for every handler call, so that nothing
    /// a module keeps in its memory survives a call. For tests. Default: off.
    pub strict: bool,
}

impl WasmSettings {
    /// These settings with the limits an extension declares in its manifest
    /// (`extension.wasm.limits`): each limit given replaces the matching
    /// setting (`memory_mib` the memory, converted from MiB; `fuel` the
    /// fuel); an absent one keeps it.
    pub fn with_limits(&self, limits: &WasmLimits) -> WasmSettings {
        let mib = |n: u64| usize::try_from(n).ok().and_then(|n| n.checked_mul(1 << 20)).unwrap_or(usize::MAX);
        let mut settings = self.clone();
        if let Some(memory) = limits.memory_mib {
            settings.memory_limit = Some(mib(memory));
        }
        if let Some(fuel) = limits.fuel {
            settings.fuel = Some(fuel);
        }
        settings
    }
}

impl Default for WasmSettings {
    fn default() -> WasmSettings {
        WasmSettings { memory_limit: Some(64 << 20), fuel: Some(1_000_000_000), strict: false }
    }
}

/// The engine host of the `wasm` binding: the settings and the console sink.
/// Cheap to clone (an `Arc` inside); every clone is the same host, and the
/// instances are keyed by it.
#[derive(Clone)]
pub struct WasmHost(Arc<HostInner>);

/// What a [`WasmHost`] holds.
struct HostInner {
    /// The host's identity.
    id: u64,
    /// The settings.
    settings: WasmSettings,
    /// Where `console` output goes.
    console: ConsoleSink,
}

/// The process-wide counter of hosts: two hosts never share instances, even
/// with equal settings.
static NEXT_HOST: AtomicU64 = AtomicU64::new(1);

impl WasmHost {
    /// A host with `settings`, and `console` written to standard error as
    /// `[wasm <level>] <text>`.
    pub fn new(settings: WasmSettings) -> WasmHost {
        WasmHost(Arc::new(HostInner {
            id: NEXT_HOST.fetch_add(1, Ordering::Relaxed),
            settings,
            console: Arc::new(|level, text| eprintln!("[wasm {level}] {text}")),
        }))
    }

    /// Routes `console` output to `sink`. When no other clone of this host
    /// exists, the identity is kept; otherwise the result is a new host, with
    /// an identity of its own, so that the clone and the result never share
    /// instances.
    pub fn with_console(self, sink: ConsoleSink) -> WasmHost {
        let mut inner = match Arc::try_unwrap(self.0) {
            Ok(inner) => inner,
            Err(shared) => HostInner {
                id: NEXT_HOST.fetch_add(1, Ordering::Relaxed),
                settings: shared.settings.clone(),
                console: Arc::clone(&shared.console),
            },
        };
        inner.console = sink;
        WasmHost(Arc::new(inner))
    }

    /// The settings every instance of this host is created with.
    pub fn settings(&self) -> &WasmSettings {
        &self.0.settings
    }

    /// Drops every instance this host's extensions created on the calling
    /// thread. Tests use it between scenarios; a program need not call it.
    pub fn discard_instances_on_this_thread(&self) {
        super::instance::discard_host(self.id());
    }

    /// How many instances of this host's extensions the calling thread holds.
    /// For the binding's tests. Not part of the interface.
    #[doc(hidden)]
    pub fn instances_on_this_thread(&self) -> usize {
        super::instance::count_host(self.id())
    }

    /// The host's identity, which keys its instances.
    pub(crate) fn id(&self) -> u64 {
        self.0.id
    }

    /// The console sink.
    pub(crate) fn console(&self) -> &ConsoleSink {
        &self.0.console
    }
}

impl fmt::Debug for WasmHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmHost").field("id", &self.0.id).field("settings", &self.0.settings).finish_non_exhaustive()
    }
}
