# Multi-Level Smart Pointer (mlsp)
The Multi-Level Smart Pointer uses an atomic global reference counter and per-thread non-atomic reference counters.

The `Mlsp` type does not implement `Send` and cannot be sent between threads, so any clone() and drop() operations performed on it use the local counter (except the last drop in a given thread).

```rust
use std::thread;

let a = Mlsp::new(1u8)

thread::spawn(move || {
    println!(a.borrow());
})
```

To send an `Mlsp` to another thread, it must be packaged creating an `MlspPackage` and incrementing the atomic reference counter.
The `MlspPackage` type does implement `Send` and is ready to be sent to other threads.
Receiving threads then use to create a new `Mlsp` with its own local reference counter.

```rust
use std::thread;

let a = Mlsp::new(1u8).package()

thread::spawn(move || {
    println!("{:?}", a.unpackage().borrow());
})
```

# Benchmarking and Testing
This library is still in need of extensive benchmarking and testing to demonstrate that it is robust and effective.
