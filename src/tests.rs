use super::*;

#[test]
fn test_block() {
    let queue = AccessQueue::new((), 2);
    assert!(!queue.block(3));
    assert!(queue.block(1));
    assert!(!queue.block(2));
    assert!(queue.block(1));
    assert!(!queue.block(1));
}

#[test]
fn test_release() {
    let queue = AccessQueue::new((), 0);
    queue.release(1);
    assert_eq!(queue.count.load(SeqCst), 1);
}

#[test]
fn holds_accesses() {
    let mut ctx = Context::from_waker(futures_task::noop_waker_ref());
    let queue = AccessQueue::new((), 1);

    let mut a1_f = queue.access();
    let mut a2_f = queue.access();

    let a1 = Pin::new(&mut a1_f).poll(&mut ctx);
    assert!(matches!(&a1, &Poll::Ready(_)));

    let a2 = Pin::new(&mut a2_f).poll(&mut ctx);
    assert!(matches!(&a2, &Poll::Pending));

    drop(a1);

    let a2 = Pin::new(&mut a2_f).poll(&mut ctx);
    assert!(matches!(&a2, &Poll::Ready(_)));
}

#[test]
fn releases_accesses_fifo() {
    let mut ctx = Context::from_waker(futures_task::noop_waker_ref());
    let queue = AccessQueue::new((), 0);

    let mut a1_f = queue.access();
    let mut a2_f = queue.access();

    let a1 = Pin::new(&mut a1_f).poll(&mut ctx);
    assert!(matches!(&a1, &Poll::Pending));

    let a2 = Pin::new(&mut a2_f).poll(&mut ctx);
    assert!(matches!(&a2, &Poll::Pending));

    queue.release(1);

    let a1 = Pin::new(&mut a1_f).poll(&mut ctx);
    assert!(matches!(&a1, &Poll::Ready(_)));

    queue.release(1);

    let a2 = Pin::new(&mut a2_f).poll(&mut ctx);
    assert!(matches!(&a2, &Poll::Ready(_)));
}

#[test]
fn reenqueue() {
    let mut ctx = Context::from_waker(futures_task::noop_waker_ref());
    let queue = AccessQueue::new((), 1);

    let mut a1_f = queue.access();
    let mut a2_f = queue.access();

    let a1 = Pin::new(&mut a1_f).poll(&mut ctx);
    assert!(matches!(&a1, &Poll::Ready(_)));

    let a2 = Pin::new(&mut a2_f).poll(&mut ctx);
    assert!(matches!(&a2, &Poll::Pending));

    if let Poll::Ready(a1) = a1 { a1.reenqueue(); } else { unreachable!() }

    let a2 = Pin::new(&mut a2_f).poll(&mut ctx);
    assert!(matches!(&a2, &Poll::Ready(_)));

    let a1 = Pin::new(&mut a1_f).poll(&mut ctx);
    assert!(matches!(&a1, &Poll::Pending));

    drop(a2);

    let a1 = Pin::new(&mut a1_f).poll(&mut ctx);
    assert!(matches!(&a1, &Poll::Ready(_)));
}

#[test]
fn hold_indefinitely_does_not_release() {
    let mut ctx = Context::from_waker(futures_task::noop_waker_ref());
    let queue = AccessQueue::new((), 1);

    let mut a1_f = queue.access();
    let mut a2_f = queue.access();

    let a1 = Pin::new(&mut a1_f).poll(&mut ctx);
    assert!(matches!(&a1, &Poll::Ready(_)));

    let a2 = Pin::new(&mut a2_f).poll(&mut ctx);
    assert!(matches!(&a2, &Poll::Pending));

    if let Poll::Ready(a1) = a1 { a1.hold_indefinitely(); } else { unreachable!() }

    let a2 = Pin::new(&mut a2_f).poll(&mut ctx);
    assert!(matches!(&a2, &Poll::Pending))
}
