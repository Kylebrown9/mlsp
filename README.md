# Multi-Level Smart Pointers
The Multi-Level Smart Pointer uses an atomic global reference counter and per-thread non-atomic reference counters.

# Mlsp
The `Mlsp` type can be used like `Rc` for sharing memory within one thread.
It does not implement `Send` and cannot be sent between threads, so any clone() and drop() operations performed on it use the local counter (except the last drop in a given thread).

```rust
let a = Mlsp::new(1u8);

thread::spawn(move || {
    let a2 = a; // WILL *NOT* WORK!!!
})
```

# MlspPackage
To send an `Mlsp` to another thread, you must make an `MlspPackage` which also increments the atomic reference counter.
The `MlspPackage` type does implement `Send` and is ready to be sent to other threads.
Receiving threads then use to create a new `Mlsp` with its own local reference counter.

```rust
let a = Mlsp::new(1u8).package();

thread::spawn(move || {
    let a2 = a; // Valid because MlspPackage implements Send
    println!("{:?}", a.unpackage().borrow());
})
```

# Benchmarking and Testing
This library is still in need of extensive benchmarking and testing to demonstrate that it is robust and effective.
