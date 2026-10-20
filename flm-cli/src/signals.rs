//! How the program ends on a signal that asks it to end, once the programs that extensions
//! run through the host service `programs` are stopped.
//!
//! On Unix, the service runs each program in a process group of its own, so that a program
//! past its time is stopped with the programs it started ([`flm_main::host_programs`]). A
//! signal sent to this program's process group then no longer reaches those programs: the
//! interrupt that Ctrl+C sends to the terminal's foreground process group (`SIGINT`), the
//! quit signal that `Ctrl+\` sends to that group (`SIGQUIT`), the hang-up sent when the
//! terminal closes (`SIGHUP`), and `SIGTERM`, which a program that supervises a command sends
//! to the group it started the command in. Under `flm watch`, `flm render` and `flm check`,
//! the program therefore handles these four signals: it stops the programs that the service
//! is running, with the programs each one started ([`stop_running_programs`]), and then ends
//! by the signal received, as a program that does not handle it ends. A calling shell thus
//! sees a command that the signal ended: it reports 128 plus the signal's number (130 for
//! Ctrl+C), and a script stops there. `flm watch` ends on the interrupt with the status 0
//! instead. A signal that the program's parent left ignored stays ignored: `nohup` ignores
//! the hang-up, and a shell ignores the interrupt for a command it runs in the background.
//!
//! On other systems, the service starts a program as any other, in this program's console,
//! and only `flm watch` handles Ctrl+C, to end with the status 0.

use std::sync::OnceLock;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};

use flm_main::host_programs::stop_running_programs;

/// How an interrupt (Ctrl+C) ends the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OnInterrupt {
    /// With the exit status 0, as `flm watch` ends.
    ExitDone,
    /// By the signal, as `flm render` and `flm check` end.
    EndBySignal,
}

/// Whether an interrupt ends the process with the status 0 ([`OnInterrupt::ExitDone`]).
#[cfg(unix)]
static INTERRUPT_EXITS_DONE: AtomicBool = AtomicBool::new(false);

/// The outcome of setting the handling, which happens once per process.
static SET: OnceLock<Result<(), String>> = OnceLock::new();

/// Sets the program to stop the programs that the host service `programs` runs when a signal
/// asks it to end, and then to end, as the [module documentation](self) describes; an
/// interrupt ends the process as `on_interrupt` says. The handling is set once per process,
/// and a later call changes how an interrupt ends the process, so that a program calling
/// [`run`](crate::run) twice is not refused.
///
/// # Errors
///
/// The system's reason, as text, when the handling cannot be set.
// See decision rationale at [§flm-dd-dr:pipe-tool-stop]
#[cfg(unix)]
pub(crate) fn stop_programs_on_signals(on_interrupt: OnInterrupt) -> Result<(), String> {
    INTERRUPT_EXITS_DONE.store(on_interrupt == OnInterrupt::ExitDone, Ordering::Relaxed);
    SET.get_or_init(handle_signals).clone()
}

/// Sets Ctrl+C to end the process with the status 0 when `on_interrupt` asks for it (`flm
/// watch`), and sets nothing otherwise. Once set, the handling stays for the process.
///
/// # Errors
///
/// The system's reason, as text, when the handling cannot be set.
#[cfg(not(unix))]
pub(crate) fn stop_programs_on_signals(on_interrupt: OnInterrupt) -> Result<(), String> {
    if on_interrupt == OnInterrupt::EndBySignal {
        return Ok(());
    }
    SET.get_or_init(|| {
        ctrlc::set_handler(|| {
            stop_running_programs();
            std::process::exit(i32::from(crate::DONE));
        })
        .map_err(|error| error.to_string())
    })
    .clone()
}

/// Starts the thread that waits for the interrupt, the quit signal, the hang-up and
/// `SIGTERM`, those of them that the program's parent did not leave ignored, and ends the
/// process on the first that arrives, once the programs that the service runs are stopped.
#[cfg(unix)]
fn handle_signals() -> Result<(), String> {
    use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
    let handled = [SIGINT, SIGQUIT, SIGTERM, SIGHUP].into_iter().filter(|signal| !ignored(*signal));
    let mut signals = signal_hook::iterator::Signals::new(handled).map_err(|error| error.to_string())?;
    std::thread::Builder::new()
        .name(String::from("flm-signals"))
        .spawn(move || {
            let Some(signal) = signals.forever().next() else { return };
            stop_running_programs();
            if signal == SIGINT && INTERRUPT_EXITS_DONE.load(Ordering::Relaxed) {
                std::process::exit(i32::from(crate::DONE));
            }
            // The signal's default action restored, the signal is raised again, and the
            // process ends by it, as it would have ended without this handling.
            let _ = signal_hook::low_level::emulate_default_handler(signal);
        })
        .map(drop)
        .map_err(|error| error.to_string())
}

/// Whether the signal `signal` is ignored, as the program's parent can leave it (`nohup`
/// ignores the hang-up; a shell ignores the interrupt for a command it runs in the
/// background). Such a signal is not handled, so it stays ignored.
#[cfg(unix)]
fn ignored(signal: libc::c_int) -> bool {
    // SAFETY: given no new action (a null pointer), `sigaction` changes nothing and writes
    // the current action of `signal` into `current`, a `sigaction` structure, for which all
    // bytes zero are a valid value; it reads and writes no other memory.
    unsafe {
        let mut current: libc::sigaction = std::mem::zeroed();
        libc::sigaction(signal, std::ptr::null(), &mut current) == 0 && current.sa_sigaction == libc::SIG_IGN
    }
}
