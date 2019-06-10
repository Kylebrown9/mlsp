![travis-ci](https://travis-ci.org/Randevelopment/mlsp.svg?branch=master)

# Multi-Level Smart Pointer (mlsp)
This project supplies an implementation of a Multi-Level Smart Pointer that attempts to bridge the gap between Rc and Arc.
When data is placed in an Mlsp a global atomic counter and a local non-atomic counter are created.
This allows us to perform cheaper non-atomic operations when Mlsp operations happen within the same thread.

Mlsp cannot be sent between threads, so any clone() and drop() operations performed on it use the local counter (except the last drop in a given thread).
When an Mlsp needs to be sent to another thread you must create a MlspPackage from it using the package method.
Creating and dropping packages increments and decrements the global counter and packages can be made into Mlsp's without affecting the global counter.

# Benchmarking and Testing
This library is still in need of extensive benchmarking and testing to demonstrate that it is robust and effective.
