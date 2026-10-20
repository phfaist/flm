//! The allocator of an engine instance: it holds the instance to its memory
//! limit and marks the instance when it refused an allocation, so that the
//! rendering of a failure tells the memory limit from a script's own
//! `throw null` by the cause, not by the value thrown.
//!
//! # Why
//!
//! QuickJS throws `null` when it runs out of memory so close to the limit
//! that it cannot allocate its own `InternalError` either, which is the usual
//! case. Its own limit (`JS_SetMemoryLimit`) refuses an allocation before any
//! allocator is called and keeps no record of it, and the memory a failed
//! handler held is freed as the exception unwinds; so nothing in the runtime
//! tells, after the failure, whether the limit was reached. This allocator
//! refuses in QuickJS's place (QuickJS's own limit is left unset) and sets
//! the mark [`Limited::reached`] reads.
//!
//! # The `unsafe` here, and why it is sound
//!
//! rquickjs's [`Allocator`] is an `unsafe trait`: an implementation must
//! answer a null pointer or a region of at least the size asked, aligned to
//! `usize`, and `usable_size` must answer the size of a region it gave.
//! [`Limited`] delegates every allocation, release and size to rquickjs's
//! [`RustAllocator`], which keeps those promises; it only answers a null
//! pointer instead of delegating when the limit would be passed (a null
//! pointer is always allowed, and a refused `realloc` leaves the region as it
//! was, as C's `realloc` does). The pointers it releases or measures are the
//! ones the delegate gave, since every region this allocator answers comes
//! from the delegate.

use std::cell::Cell;
use std::rc::Rc;

use rquickjs::allocator::{Allocator, RustAllocator};

/// An engine instance's allocator: [`RustAllocator`], counting the bytes it
/// holds and refusing an allocation that would pass `limit`.
pub(crate) struct Limited {
    /// The limit in bytes (`usize::MAX` when there is none).
    limit: usize,
    /// The bytes the regions given and not released hold.
    used: usize,
    /// Set when an allocation was refused; cleared by the engine at the start
    /// of each handler call ([`Engine::start_budget`](super::engine::Engine::start_budget)).
    reached: Rc<Cell<bool>>,
}

impl Limited {
    /// An allocator holding an instance to `limit` bytes (none when `None`),
    /// and the mark it sets when it refuses an allocation.
    pub(crate) fn new(limit: Option<usize>) -> (Self, Rc<Cell<bool>>) {
        let reached = Rc::new(Cell::new(false));
        (Limited { limit: limit.unwrap_or(usize::MAX), used: 0, reached: Rc::clone(&reached) }, reached)
    }

    /// Whether `more` bytes fit under the limit; marks the instance when not.
    fn fits(&self, more: usize) -> bool {
        let fits = self.used.checked_add(more).is_some_and(|total| total <= self.limit);
        if !fits {
            self.reached.set(true);
        }
        fits
    }

    /// Counts the region `ptr` the delegate answered (none when null).
    fn count(&mut self, ptr: *mut u8) -> *mut u8 {
        if !ptr.is_null() {
            // SAFETY: `ptr` is a region the delegate just gave.
            self.used += unsafe { RustAllocator::usable_size(ptr) };
        }
        ptr
    }
}

// SAFETY: see the module's documentation; every region comes from
// `RustAllocator`, and a refusal answers a null pointer.
unsafe impl Allocator for Limited {
    fn alloc(&mut self, size: usize) -> *mut u8 {
        if !self.fits(size) {
            return std::ptr::null_mut();
        }
        let ptr = RustAllocator.alloc(size);
        self.count(ptr)
    }

    fn calloc(&mut self, count: usize, size: usize) -> *mut u8 {
        if !count.checked_mul(size).is_some_and(|total| self.fits(total)) {
            return std::ptr::null_mut();
        }
        let ptr = RustAllocator.calloc(count, size);
        self.count(ptr)
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8) {
        // SAFETY: `ptr` is a region of this allocator, so of the delegate.
        unsafe {
            self.used = self.used.saturating_sub(RustAllocator::usable_size(ptr));
            RustAllocator.dealloc(ptr);
        }
    }

    unsafe fn realloc(&mut self, ptr: *mut u8, new_size: usize) -> *mut u8 {
        // SAFETY: `ptr` is a region of this allocator, so of the delegate.
        unsafe {
            let old = RustAllocator::usable_size(ptr);
            if new_size > old && !self.fits(new_size - old) {
                return std::ptr::null_mut();
            }
            let moved = RustAllocator.realloc(ptr, new_size);
            if !moved.is_null() {
                self.used = self.used.saturating_sub(old);
            }
            self.count(moved)
        }
    }

    unsafe fn usable_size(ptr: *mut u8) -> usize {
        // SAFETY: `ptr` is a region of this allocator, so of the delegate.
        unsafe { RustAllocator::usable_size(ptr) }
    }
}
