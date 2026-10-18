//! The workaround for the crash after the game window is dragged.
//!
//! `render.execute_view` loads the render context's prepared-view pointer and dereferences it
//! without checking it. The view setup stores that pointer and finalize clears it, and after
//! the window has been dragged — moved, resized, restored — the engine has been seen
//! executing a view whose pointer was already cleared, which faults inside the engine.
//!
//! Both of the engine's call sites ignore what `execute_view` returns, so the fix is to not
//! make the call: the frame loses that one view and the game carries on. Which is the whole
//! reason this is possible from a plugin at all.

// This module exists to stand in for a game function; the detour and the read of the game's
// own memory cannot be expressed in safe Rust.
#![allow(unsafe_code)]

use core::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use dayz_game::Dayz;
use uniloader_sdk::{Host, PluginError};

/// `render.execute_view`, as the engine declares it.
///
/// `__fastcall` on x86-64 is the one Windows calling convention, so `extern "C"` here is the
/// same ABI. `mode` arrives in `r8b` and the engine reads only that byte.
type ExecuteView = unsafe extern "C" fn(*mut c_void, *mut c_void, u8) -> *mut c_void;

/// The trampoline that calls the original, as a plain address.
///
/// Atomics rather than a `Mutex` throughout: the engine calls the detour from the render
/// thread, several times a frame, and must never wait on a lock the console is holding.
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// `render.prepared_view`, the offset of the pointer inside the render context.
static OFFSET: AtomicU64 = AtomicU64::new(0);
/// Whether the guard skips the call, from the setting of the same name.
static ENABLED: AtomicBool = AtomicBool::new(true);
/// How many calls were skipped since the plugin started.
static SKIPPED: AtomicU64 = AtomicU64::new(0);
/// How many calls went through.
static FORWARDED: AtomicU64 = AtomicU64::new(0);

/// Every skip is logged until this many have been seen, then every hundredth, so a condition
/// that persists cannot fill the log at frame rate.
const VERBOSE_SKIPS: u64 = 10;
/// How often a skip is logged once past [`VERBOSE_SKIPS`].
const SKIP_INTERVAL: u64 = 100;

/// Install the detour. The loader holds the original bytes and puts them back on stop.
///
/// # Errors
/// The symbol or the offset is missing from the database for this build, or the address is
/// one the loader refuses to detour.
pub(crate) fn install(host: &Host, dayz: &Dayz, enabled: bool) -> Result<(), PluginError> {
    let target = dayz.symbol("render.execute_view")?;
    let offset = dayz.offset("render.prepared_view")?;
    OFFSET.store(offset, Ordering::Relaxed);
    ENABLED.store(enabled, Ordering::Relaxed);
    // SAFETY: `guarded` has the signature of the function the database names, which is what
    // the symbol stands for; the loader checks the address itself.
    let (_hook, original) = unsafe {
        host.detour(
            target,
            guarded as ExecuteView as *mut c_void,
            "guard execute_view against a cleared prepared view",
        )?
    };
    ORIGINAL.store(original as usize, Ordering::Release);
    log::info!(
        "guarding render.execute_view at {target:p}, prepared view at +{offset:#x}, guard {}",
        if enabled { "on" } else { "off" }
    );
    Ok(())
}

/// Turn the guard on or off without removing the detour.
///
/// Off, the detour still stands and still counts; it simply stops skipping. That way a user
/// who suspects the guard of eating views can switch it off and compare without a restart.
pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Counters for the `status` command: forwarded, skipped, and whether the guard is on.
pub(crate) fn counters() -> (u64, u64, bool) {
    (
        FORWARDED.load(Ordering::Relaxed),
        SKIPPED.load(Ordering::Relaxed),
        ENABLED.load(Ordering::Relaxed),
    )
}

/// What stands in for `render.execute_view`.
unsafe extern "C" fn guarded(engine: *mut c_void, context: *mut c_void, mode: u8) -> *mut c_void {
    if skip(context) {
        let skips = SKIPPED.fetch_add(1, Ordering::Relaxed) + 1;
        if skips <= VERBOSE_SKIPS || skips.is_multiple_of(SKIP_INTERVAL) {
            log::warn!(
                "skipped a view the engine never prepared: context {context:p}, mode {mode}, \
                 {skips} so far"
            );
        }
        // Null is what the engine's own early-out returns, and neither call site looks.
        return core::ptr::null_mut();
    }
    FORWARDED.fetch_add(1, Ordering::Relaxed);
    let original = ORIGINAL.load(Ordering::Acquire);
    if original == 0 {
        // The detour is installed before the trampoline address is stored, so a call that
        // lands in between has nothing to forward to. Dropping one view beats a null call.
        return core::ptr::null_mut();
    }
    // SAFETY: `original` is the trampoline the loader returned for this exact function, so it
    // has this signature, and the arguments are the engine's own, passed straight through.
    unsafe {
        let original: ExecuteView = core::mem::transmute::<usize, ExecuteView>(original);
        original(engine, context, mode)
    }
}

/// Whether this call has to be skipped: the guard is on and the prepared view is null.
fn skip(context: *mut c_void) -> bool {
    if !ENABLED.load(Ordering::Relaxed) {
        return false;
    }
    if context.is_null() {
        return true;
    }
    // A read that faulted is also a skip: whatever that context is, the engine is about to
    // dereference a pointer out of it.
    prepared_view(context).is_none()
}

/// The prepared-view pointer, or `None` when reading it faulted.
///
/// The context comes from the engine's own call and is readable; the structured handler is
/// for the case the workaround exists for, a context being torn down on another thread.
fn prepared_view(context: *mut c_void) -> Option<core::ptr::NonNull<c_void>> {
    let offset = usize::try_from(OFFSET.load(Ordering::Relaxed)).ok()?;
    let mut value: *mut c_void = core::ptr::null_mut();
    let slot = &raw mut value;
    // SAFETY: `context` is non-null and the offset is the one the database gives for this
    // build; a read of a context that has gone away is contained by the handler, which is
    // the whole point of doing it here.
    let read = microseh::try_seh(|| unsafe {
        slot.write(
            context
                .byte_add(offset)
                .cast::<*mut c_void>()
                .read_volatile(),
        );
    });
    if read.is_err() {
        return None;
    }
    core::ptr::NonNull::new(value)
}
