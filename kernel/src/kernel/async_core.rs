//! Reactive Async Core
//!
//! A minimal, no_std async executor for the Intent Kernel.
//! This enables the "Reactive" part of the forward-looking architecture.
//! Now with True Sleeping (WFI) support!

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use core::sync::atomic::{AtomicBool, Ordering};
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use crate::arch::{self, SpinLock};

// ═══════════════════════════════════════════════════════════════════════════════
// EXECUTOR
// ═══════════════════════════════════════════════════════════════════════════════

pub struct Executor {
    tasks: VecDeque<Arc<Task>>,
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct Task {
    future: SpinLock<BoxFuture<'static, ()>>,
    woken: AtomicBool,
}

impl Task {
    fn wake(self: &Arc<Self>) {
        self.woken.store(true, Ordering::SeqCst);
    }
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, future: impl Future<Output = ()> + 'static + Send) {
        let task = Arc::new(Task {
            future: SpinLock::new(Box::pin(future)),
            woken: AtomicBool::new(true), // Start as woken
        });
        self.tasks.push_back(task);
    }

    pub fn run(&mut self) {
        loop {
            let mut did_work = false;
            let count = self.tasks.len();
            
            for _ in 0..count {
                if let Some(task) = self.tasks.pop_front() {
                    // Check if woken
                    if task.woken.load(Ordering::SeqCst) {
                        // Clear woken flag
                        task.woken.store(false, Ordering::SeqCst);
                        
                        // Create waker
                        let waker = waker_ref(&task);
                        let mut context = Context::from_waker(&waker);
                        
                        // Poll future
                        let mut future_guard = task.future.lock();
                        match future_guard.as_mut().poll(&mut context) {
                            Poll::Ready(()) => {
                                // Task finished, don't push back
                                did_work = true;
                            }
                            Poll::Pending => {
                                // Task pending, push back
                                drop(future_guard);
                                self.tasks.push_back(task);
                                did_work = true; // We did a poll
                            }
                        }
                    } else {
                        // Not woken, just push back
                        self.tasks.push_back(task);
                    }
                }
            }
            
            // If no tasks were ready to run, sleep
            if !did_work {
                // Wait for interrupt
                unsafe { arch::wfi(); }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// WAKER IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════════

fn waker_ref(task: &Arc<Task>) -> Waker {
    let ptr = Arc::into_raw(task.clone()) as *const ();
    
    unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
        let arc = Arc::from_raw(ptr as *const Task);
        core::mem::forget(arc.clone()); // Increment ref count
        let new_ptr = Arc::into_raw(arc) as *const ();
        RawWaker::new(new_ptr, &VTABLE)
    }

    unsafe fn wake(ptr: *const ()) {
        let arc = Arc::from_raw(ptr as *const Task);
        arc.wake();
        // Drop arc
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        let arc = core::mem::ManuallyDrop::new(Arc::from_raw(ptr as *const Task));
        arc.wake();
    }

    unsafe fn drop_waker(ptr: *const ()) {
        drop(Arc::from_raw(ptr as *const Task));
    }

    static VTABLE: RawWakerVTable = RawWakerVTable::new(
        clone_waker,
        wake,
        wake_by_ref,
        drop_waker,
    );

    unsafe { Waker::from_raw(RawWaker::new(ptr, &VTABLE)) }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ASYNC PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════════

/// Yield execution once
pub struct YieldNow {
    yielded: bool,
}

impl YieldNow {
    pub fn new() -> Self {
        YieldNow { yielded: false }
    }
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

pub async fn yield_now() {
    YieldNow::new().await
}
