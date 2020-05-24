use std::future::Future;
use std::mem;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::task::{Context, Poll};

use futures_core::ready;
use event_listener::{Event, EventListener};

pub struct AccessQueue<T> {
    count: AtomicUsize,
    event: Event,
    inner: T,
}

impl<T> AccessQueue<T> {
    pub fn new(inner: T, count: usize) -> AccessQueue<T> {
        AccessQueue {
            count: AtomicUsize::new(count),
            event: Event::new(),
            inner,
        }
    }

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

    pub fn release(&self, amt: usize) {
        self.count.fetch_add(amt, SeqCst);
        self.event.notify(amt);
    }

    pub fn access(&self) -> Access<'_, T> {
        Access {
            queue: self,
            listener: None,
        }
    }

    pub fn skip_queue(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

pub struct Access<'a, T> {
    queue: &'a AccessQueue<T>,
    listener: Option<EventListener>,
}

impl<'a, T> Future for Access<'a, T> {
    type Output = &'a T;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(listener) = &mut self.listener {
            ready!(Pin::new(listener).poll(ctx));
            self.listener = None;
        }

        while !self.queue.block(1) {
            match &mut self.listener {
                Some(listener)  => ready!(Pin::new(listener).poll(ctx)),
                None            => {
                    let mut listener = self.queue.event.listen();
                    if let Poll::Pending = Pin::new(&mut listener).poll(ctx) {
                        self.listener = Some(listener);
                        return Poll::Pending
                    }
                }
            }
        }

        Poll::Ready(self.queue.skip_queue())
    }
}

pub struct AccessGuard<'a, T> {
    queue: &'a AccessQueue<T>,
}

impl<'a, T> AccessGuard<'a, T> {
    pub fn hold_indefinitely(self) -> &'a T {
        let inner = self.queue.skip_queue();
        mem::forget(self);
        inner
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
