use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crate::arch;

// ═══════════════════════════════════════════════════════════════════════════════
// RAW SPINLOCK (No Tracking - Safe for Allocator/Registry)
// ═══════════════════════════════════════════════════════════════════════════════

/// Raw SpinLock that does not track ownership or deadlocks.
/// Use this ONLY for:
/// 1. The Allocator
/// 2. The LockRegistry itself
/// 3. Lowest-level architectural primitives
pub struct RawSpinLock<T: ?Sized> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for RawSpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for RawSpinLock<T> {}

impl<T> RawSpinLock<T> {
    pub const fn new(data: T) -> Self {
        RawSpinLock {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> RawSpinLock<T> {
    pub fn lock(&self) -> RawSpinLockGuard<'_, T> {
        // Disable interrupts
        let saved_int_state = arch::irq_disable();
        
        while self.lock.compare_exchange_weak(
            false, true, 
            Ordering::Acquire, 
            Ordering::Relaxed
        ).is_err() {
            while self.lock.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        RawSpinLockGuard { 
            lock: self,
            saved_int_state,
        }
    }
    
    pub fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}

pub struct RawSpinLockGuard<'a, T: ?Sized> {
    lock: &'a RawSpinLock<T>,
    saved_int_state: u64,
}

impl<'a, T: ?Sized> Deref for RawSpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for RawSpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for RawSpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock();
        arch::irq_restore(self.saved_int_state);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LOCK REGISTRY
// ═══════════════════════════════════════════════════════════════════════════════

struct LockInfo {
    holder: Option<u64>,  // PID
    waiters: Vec<u64>,    // PIDs
}

/// Global registry of all active locks and their state
/// Protected by a RawSpinLock to avoid recursion
static LOCK_REGISTRY: RawSpinLock<BTreeMap<usize, LockInfo>> = 
    RawSpinLock::new(BTreeMap::new());

/// Register a new lock
fn register_lock(id: usize) {
    // We need to be careful. BTreeMap allocation uses GlobalAllocator.
    // GlobalAllocator uses RawSpinLock.
    // So this is safe from recursion IF LockRegistry uses RawSpinLock.
    // BUT, if we are in an interrupt handler that interrupted the Allocator...
    // RawSpinLock disables interrupts, so that's safe.
    
    // However, we can't allocate if we are IN the allocator.
    // The Allocator uses RawSpinLock.
    // If we use SpinLock (Tracked) in the Allocator, we trigger this.
    // But we decided Allocator uses RawSpinLock.
    // So Allocator calls will NOT call register_lock.
    // So this is safe.
    
    let mut registry = LOCK_REGISTRY.lock();
    registry.insert(id, LockInfo { holder: None, waiters: Vec::new() });
}

/// Record that a task is waiting for a lock
fn record_wait(lock_id: usize, pid: u64) {
    let mut registry = LOCK_REGISTRY.lock();
    if let Some(info) = registry.get_mut(&lock_id) {
        if !info.waiters.contains(&pid) {
            info.waiters.push(pid);
        }
    }
}

/// Record that a task acquired a lock
fn record_acquire(lock_id: usize, pid: u64) {
    let mut registry = LOCK_REGISTRY.lock();
    if let Some(info) = registry.get_mut(&lock_id) {
        info.holder = Some(pid);
        // Remove from waiters
        if let Some(pos) = info.waiters.iter().position(|&x| x == pid) {
            info.waiters.remove(pos);
        }
    }
}

/// Record that a task released a lock
fn record_release(lock_id: usize) {
    let mut registry = LOCK_REGISTRY.lock();
    if let Some(info) = registry.get_mut(&lock_id) {
        info.holder = None;
    }
}

/// Get the current wait graph (for deadlock detection)
pub fn get_wait_graph() -> BTreeMap<u64, Vec<u64>> {
    let registry = LOCK_REGISTRY.lock();
    let mut graph = BTreeMap::new();
    
    for (_, info) in registry.iter() {
        if let Some(holder) = info.holder {
            for &waiter in &info.waiters {
                graph.entry(waiter).or_insert_with(Vec::new).push(holder);
            }
        }
    }
    
    graph
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRACKED SPINLOCK
// ═══════════════════════════════════════════════════════════════════════════════

static NEXT_LOCK_ID: AtomicUsize = AtomicUsize::new(1);

/// SpinLock with Deadlock Detection Tracking
pub struct SpinLock<T: ?Sized> {
    lock: AtomicBool,
    id: AtomicUsize,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        SpinLock {
            lock: AtomicBool::new(false),
            id: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> SpinLock<T> {
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        let saved_int_state = arch::irq_disable();
        
        // Get current PID
        let core_id = arch::core_id();
        let pid = if core_id < 4 {
            crate::kernel::scheduler::CURRENT_PIDS[core_id as usize].load(Ordering::Relaxed)
        } else {
            0
        };
        
        // Lazy ID initialization
        let mut lock_id = self.id.load(Ordering::Relaxed);
        if lock_id == 0 {
            let new_id = NEXT_LOCK_ID.fetch_add(1, Ordering::Relaxed);
            match self.id.compare_exchange(0, new_id, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => {
                    lock_id = new_id;
                    register_lock(lock_id);
                }
                Err(actual) => lock_id = actual,
            }
        }
        
        // Try to acquire
        if self.lock.compare_exchange_weak(
            false, true, 
            Ordering::Acquire, 
            Ordering::Relaxed
        ).is_ok() {
            // Register acquisition
            if pid != 0 {
                record_acquire(lock_id, pid as u64);
            }
            return SpinLockGuard { 
                lock: self,
                saved_int_state,
                pid: pid as u64,
                lock_id,
            };
        }
        
        // Contention! Record wait
        if pid != 0 {
            record_wait(lock_id, pid as u64);
        }
        
        // Spin
        while self.lock.compare_exchange_weak(
            false, true, 
            Ordering::Acquire, 
            Ordering::Relaxed
        ).is_err() {
            while self.lock.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        
        // Acquired
        // Acquired
        if pid != 0 {
            record_acquire(lock_id, pid as u64);
        }
        
        SpinLockGuard { 
            lock: self,
            saved_int_state,
            pid: pid as u64,
            lock_id,
        }
    }
    
    pub fn unlock(&self, pid: u64, lock_id: usize) {
        if pid != 0 && lock_id != 0 {
            record_release(lock_id);
        }
        self.lock.store(false, Ordering::Release);
    }
}

pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
    saved_int_state: u64,
    pid: u64,
    lock_id: usize,
}

impl<'a, T: ?Sized> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock(self.pid, self.lock_id);
        arch::irq_restore(self.saved_int_state);
    }
}
