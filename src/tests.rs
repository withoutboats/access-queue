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
