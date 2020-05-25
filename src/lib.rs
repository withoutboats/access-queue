//! [`AccessQueue`], which allows only a certain number of simultaneous accesses to a guarded item.
//!
//! This can be useful for implementing backpressure: when accessing the item through the
//! [`Access`] future, tasks will wait to access the item until others have completed, limiting the
//! number of accesses that occur at the same time.
#![deny(warnings, missing_debug_implementations, missing_docs, rust_2018_idioms)]
use std::future::Future;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::task::{Context, Poll};

use futures_core::ready;
use event_listener::{Event, EventListener};

#[cfg(test)]
mod tests;

/// The AccessQueue which guards access to some item.
#[derive(Debug)]
pub struct AccessQueue<T> {
    count: AtomicUsize,
    event: Event,
    inner: T,
}

impl<T> AccessQueue<T> {
    /// Construct a new `AccessQueue`, which guards the `inner` value and allows only `count`
    /// concurrent accesses to occur simultaneously.
    pub fn new(inner: T, count: usize) -> AccessQueue<T> {
        AccessQueue {
            count: AtomicUsize::new(count),
            event: Event::new(),
            inner,
        }
    }

    /// Block `amt` accesses.
    ///
    /// This reduces the number of concurrent accesses to the guarded item that are allowed. Until
    /// `release` is called, this many accesses are blocked from occurring.
    pub fn block(&self, amt: usize) -> bool {
        let mut current = self.count.load(SeqCst);
        while current >= amt {
            match self.count.compare_exchange_weak(current, current - amt, SeqCst, SeqCst) {
                Ok(_)   => return true,
                Err(n)  => current = n,
            }
        }
        false
    }

    /// Release `amt` additional accesses.
    ///
    /// This increases the number of concurrent accesses to the guarded item that are alloewd. It
    /// can be paired with `block` to raise and lower the limit.
    pub fn release(&self, amt: usize) {
        self.count.fetch_add(amt, SeqCst);
        self.event.notify_additional(amt);
    }

    /// Wait in the queue to access the guarded item.
    pub fn access(&self) -> Access<'_, T> {
        Access {
            queue: self,
            listener: None,
        }
    }

    /// Skip the access queue and get a reference to the inner item.
    ///
    /// This does not modify the number of simultaneous accesses allowed. It can be useful if the
    /// AccessQueue is only limited certain patterns of use on the inner item.
    pub fn skip_queue(&self) -> &T {
        &self.inner
    }

    /// Get the inner item mutably.
    ///
    /// This requires mutable access to the AccessQueue, guaranteeing that no simultaneous accesses
    /// are occurring.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

/// A `Future` of a queued access to the inner item.
///
/// This can be constructed from [`AccessQueue::access`]. It is a `Future`, and it resolves to an
/// [`AccessGuard`], which dereferences to the inner item guarded by the access queue.
#[derive(Debug)]
pub struct Access<'a, T> {
    queue: &'a AccessQueue<T>,
    listener: Option<EventListener>,
}

impl<'a, T> Access<'a, T> {
    /// Access the guarded item without waiting in the `AccessQueue`.
    ///
    /// This can be used to access the item without following the limitations on the number of
    /// allowed concurrent accesses.
    pub fn skip_queue(&self) -> &T {
        self.queue.skip_queue()
    }
}

impl<'a, T> Future for Access<'a, T> {
    type Output = AccessGuard<'a, T>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(listener) = &mut self.listener {
            ready!(Pin::new(listener).poll(ctx));
            self.listener = None;
        }

        while !self.queue.block(1) {
            match &mut self.listener {
                Some(listener)  => {
                    ready!(Pin::new(listener).poll(ctx));
                    self.listener = None;
                }
                None            => {
                    let mut listener = self.queue.event.listen();
                    if let Poll::Pending = Pin::new(&mut listener).poll(ctx) {
                        self.listener = Some(listener);
                        return Poll::Pending
                    }
                }
            }
        }

        Poll::Ready(AccessGuard { queue: self.queue })
    }
}

/// A resolved access to the guarded item.
#[derive(Debug)]
pub struct AccessGuard<'a, T> {
    queue: &'a AccessQueue<T>,
}

impl<'a, T> AccessGuard<'a, T> {
    /// Hold this guard indefinitely, without ever releasing it.
    ///
    /// Normaly, when an `AccessGuard` drops, it releases one access in the `AccessQueue` so that
    /// another `Access` can resolve. If this method is called, instead it is downgraded into a
    /// normal reference and the access is never released.
    pub fn hold_indefinitely(self) -> &'a T {
        ManuallyDrop::new(self).queue.skip_queue()
    }
}

impl<'a, T> Deref for AccessGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.queue.skip_queue()
    }
}

impl<'a, T> Drop for AccessGuard<'a, T> {
    fn drop(&mut self) {
        self.queue.release(1);
    }
}

#[allow(dead_code)]
fn is_send_sync<T: Send + Sync>() where AccessQueue<T>: Send + Sync { }
