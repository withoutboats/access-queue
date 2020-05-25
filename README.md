An access queue allows at most `N` concurrent accesses to guarded type. It is an async concurrency
primitive intended to support certain backpressure patterns.

```rust
// This queue limits the number of simultaneous references to `inner` to 4
let queue = AccessQueue::new(inner, 4);

// get an inner reference
let inner1 = queue.access().await;

// get more (from other concurrent tasks)
let inner2 = queue.access().await;
let inner3 = queue.access().await;
let inner4 = queue.access().await;


// this access has to wait, because there are already 4 accesses ongoing
// (note: you should not call access multiple times from within the same
//  scope. this example is meant to simulate accessing from multiple
//  tasks concurrently)
let inner5 = queue.access().await;
```

When constructing an access queue, users set how many accesses are allowed to occur at the same
time. Then, using the `AccessQueue::access` API, they get a future which resolves to a type that
dereferences to the guarded value. This limits how many accesses can occur simultaneously, causing
accesses beyond the limit to wait until previous accesses have completed.

Accesses are always processed in a first-in, first-out order. Accesses which are awaited first get
access before accesses which are awaited later.

The normal `AccessQueue::access` API manages accesses in simple patterns, but more complex patterns
are supported as well:

* With `AccessQueue::block` and `AccessQueue::release`, you can augment the number of accesses
  without actually accessing the value, blocking some accesses or releasing more.
* With `AccessQueue::skip_queue` you can access the inner value without waiting in the queue at
  all.
* With `AccessGuard::hold_indefinitely` you can keep an access even after it drops from scope,
  never releasing it back to the queue.

## Safety

`AccessQueue` is a concurrency primitive, but it does not allow mutable access to any value because
it never guarantees that any access is exclusive. For this reason, `AccessQueue` is 100% safe coe,
because its correctness properties do not have anything to do with memory safety.
