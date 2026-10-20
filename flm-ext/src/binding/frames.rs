//! The thread's frames: the state of the handler calls running on this thread,
//! innermost last. The one module of the crate's shared binding code whose own
//! code holds `unsafe` (invariant 5 lists every site of the crate).
//!
//! # Why
//!
//! A native function the engine calls (the glue) must reach the call's handle
//! table, which lives on the stack of the Rust function running the handler.
//! A closure given to rquickjs's `Function::new` cannot borrow it (its bound is
//! the `'js` lifetime of `Context::with`'s closure, which a borrow from outside
//! cannot be proven to outlive), and `Context::with` cannot be entered again on
//! a thread while it runs (it borrows the runtime's `RefCell` mutably). So the
//! running call is recorded here, on a thread-local stack, as a pointer whose
//! lifetime is erased: the table. (The live context a nested entry into a
//! QuickJS engine needs is kept by that engine itself, not by a frame.) The
//! engines `wasm` and `native` use the same frames: a guest's request reaches
//! the table of the call it was sent from through them, while the guest is
//! suspended in its request (in the import `flm.call`, or, compiled into the
//! program, in the guest client's `abi::native`).
//!
//! # The invariants that make the `unsafe` blocks sound
//!
//! 1. A frame is pushed by [`enter`] inside [`Binding::enter`]'s closure (the
//!    one `with_engine` runs, for QuickJS), after the table exists, and popped
//!    by [`enter`]'s guard before it goes away (also when the closure
//!    unwinds). The table outlives [`enter`] (it is on the stack of the
//!    function that calls [`enter`]).
//! 2. Only the innermost frame's table is read, and only for a native of the
//!    frame's own extension: [`with_current`] compares the loaded extension
//!    that sent the native with the frame's ([`Binding::id`]) before it
//!    borrows the table. A nested handler call on this thread can only start
//!    from inside a native call of the innermost frame (`flm-core` calls the
//!    next handler from within the method the glue dispatched), so the outer
//!    frame's table is not in use while the inner frame is live.
//!    [`with_current`] releases the frame stack's borrow before it runs its
//!    closure, so a nested call can push its frame. The closure runs no code
//!    of the extension (JavaScript, a guest's WebAssembly, or a native
//!    extension's Rust) of the frame it reads: an extension's code runs again
//!    only in a nested handler call, which pushes a frame of its own (with
//!    its own table). Two cases run extension code inside the closure before
//!    any new frame exists, and in both a native that code sends comes from a
//!    loaded extension with no handler call running on this thread, so it
//!    finds a frame of a different loaded extension innermost (or no frame),
//!    and [`with_current`] refuses it without borrowing the table:
//!    - the nested call of another extension that has no instance on this
//!      thread yet creates one, and the new instance's `define` (for QuickJS,
//!      the script's top-level code) runs before that call's frame is pushed;
//!      an instance is created on a thread only while no handler call of its
//!      loaded extension runs there, since a nested entry reuses the running
//!      instance;
//!    - natively, the `Drop` of an object a native extension's instance
//!      recorded is the extension's code too, and the engine `native` drops
//!      an instance outside its entries (after a panic, at the outermost
//!      entry into it, possibly inside another extension's native; an
//!      instance of an extension no longer held; an instance whose `define`
//!      failed, or whose load's `packages` failed), with the instance's own
//!      state current, so that its natives
//!      are its own loaded extension's; no handler call of that loaded
//!      extension runs on the thread then (`native/instance.rs`, "Dropping an
//!      instance").
//!
//!    So no two `&mut` to one table are ever live.
//! 3. An entry of a table borrows only what lives for the whole call (plan A,
//!    A15). [`with_current`]'s closure is generic over the table's lifetime as
//!    well as the view's, so it can lend into the table only owned data or what
//!    it takes from the table's own entries: never a borrow of the view (its
//!    `registration`), which ends with the closure.
//! 4. Nothing is `Send`: the frame stack is thread-local; a frame never leaves
//!    its thread; the raw pointer keeps the erased frame out of `Send` by
//!    construction.
//! 5. `grep -rn unsafe flm-ext/src` finds the block of [`with_current`] here,
//!    `quickjs/engine.rs`'s one call of `Ctx::from_raw` in `reenter` (the
//!    engine's live context, whose own invariant is stated there), and the
//!    allocator `Limited` of `quickjs/memory.rs`; and, compiled only for
//!    `wasm32` without the feature `native`, the guest client's import block
//!    in `guest/abi.rs` (`unsafe extern "C"`, its two functions declared
//!    `safe`, whose account is given there),
//!    and, in an extension's crate, the attribute `#[unsafe(no_mangle)]` on
//!    the four exports the guest client's macro `extension!` writes
//!    (`guest/mod.rs`), each a call of a safe function of `guest/abi.rs`; the
//!    adapter, the API and the runtime hold none.
//!
//! [`Binding::enter`]: super::Binding::enter
//! [`Binding::id`]: super::Binding::id

use std::cell::{Cell, RefCell};

use super::Registration;
use crate::adapter::handles::CallTable;

/// One handler call on this thread. Lives on the stack of the function that
/// runs the handler; the frame stack holds its pointer while the handler
/// runs. `'f` is the borrow of the table, `'c` the call's lifetime (the
/// table's).
pub(crate) struct Frame<'f, 'c> {
    /// The call's number, the table's.
    pub call: u64,
    /// The registration the call belongs to: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The call's handle table.
    pub table: &'f mut CallTable<'c>,
}

/// What [`with_current`] hands its closure: the innermost frame, its table
/// borrowed for the closure's run. `'v` is the view's lifetime, `'c` the
/// table's (the call's), which the closure cannot name (invariant 3).
pub(crate) struct FrameView<'v, 'c> {
    /// The call's number.
    #[allow(dead_code)] // part of the frame's record; no native reads it
    pub call: u64,
    /// The registration the call belongs to.
    pub registration: &'v Registration,
    /// The call's handle table.
    pub table: &'v mut CallTable<'c>,
}

/// A frame with its lifetimes erased, as the frame stack holds it.
struct ErasedFrame {
    call: u64,
    registration: Registration,
    table: *mut CallTable<'static>,
}

thread_local! {
    /// The frames of the handler calls running on this thread, innermost last.
    static FRAMES: RefCell<Vec<ErasedFrame>> = const { RefCell::new(Vec::new()) };
    /// The next call number on this thread.
    static NEXT_CALL: Cell<u64> = const { Cell::new(1) };
}

/// Pops the innermost frame when dropped.
struct PopGuard;

impl Drop for PopGuard {
    fn drop(&mut self) {
        FRAMES.with(|frames| {
            frames.borrow_mut().pop();
        });
    }
}

/// Pushes `frame`, runs `f`, pops the frame (also when `f` unwinds: a guard).
pub(crate) fn enter<R>(frame: Frame<'_, '_>, f: impl FnOnce() -> R) -> R {
    // Erasing the lifetimes is a pointer cast; nothing else changes. The
    // pointer is dereferenced only by `with_current`, under invariants 1 to 3.
    let table = (frame.table as *mut CallTable<'_>).cast::<CallTable<'static>>();
    FRAMES.with(|frames| {
        frames.borrow_mut().push(ErasedFrame { call: frame.call, registration: frame.registration, table });
    });
    let _guard = PopGuard;
    f()
}

/// The innermost frame, for a native that the loaded extension of number
/// `load` ([`Binding::id`](super::Binding::id)) sent: `None` when no handler
/// runs on this thread (a native called from a module's top level, say), or
/// when the innermost frame is a call of another loaded extension (a native
/// sent while an instance of the extension `load` is created inside that
/// call). The loaded extensions are compared before the frame's table is
/// borrowed (invariant 2).
pub(crate) fn with_current<R>(load: u64, f: impl for<'v, 'c> FnOnce(&mut FrameView<'v, 'c>) -> R) -> Option<R> {
    // Copy what the view needs and release the borrow of the frame stack, so
    // that a nested handler call started from within `f` can push its frame.
    let (call, registration, table) = FRAMES.with(|frames| {
        frames.borrow().last().map(|top| (top.call, top.registration.clone(), top.table))
    })?;
    if registration.binding.id() != load {
        return None;
    }
    // SAFETY: `table` points to the table of the innermost frame, which is
    // live: the frame was pushed by `enter` after the table was created and is
    // popped by `enter`'s guard before the table goes away (invariant 1). The
    // innermost frame's table is the only one in use (invariant 2): the native
    // was sent by the frame's own extension (checked above), whose code that
    // sent it is suspended in this frame's call, so no other reference to the
    // table exists while `f` runs; `f` runs no code of this frame's extension,
    // which runs again only in a nested frame pushed from within `f`, whose
    // own table is a different one, and a native that another extension sends
    // from within `f` (an instance being created) is refused above, before any
    // borrow. The borrow's lifetime is local to this call, and the table's
    // lifetime parameter is one `f` cannot name: `f` is generic over both
    // (invariant 3), so it can neither keep the reference beyond its own run
    // nor lend into the table anything that does not live as long as the
    // table's own entries (A15).
    let table: &mut CallTable<'_> = unsafe { &mut *table.cast::<CallTable<'_>>() };
    let mut view = FrameView { call, registration: &registration, table };
    Some(f(&mut view))
}

/// A fresh call number on this thread.
pub(crate) fn next_call() -> u64 {
    NEXT_CALL.with(|next| {
        let call = next.get();
        next.set(call.wrapping_add(1));
        call
    })
}
