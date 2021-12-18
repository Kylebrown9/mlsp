use core::cell::Cell;
use core::ptr;
use core::sync::atomic;

use std::borrow::Borrow;
use std::boxed::Box;
use std::sync::atomic::AtomicUsize;

use std::sync::atomic::Ordering;

/// The inner Arc-like portion of the Mlsp
/// It is a wrapper tha bundles an atomic usize reference counter
/// with an arbitrary value
struct MlspData<T> {
    atomic_count: AtomicUsize,
    value: T
}

impl<T> MlspData<T> {
    /// Creates a new data bundle with an atomic counter with value 1
    fn new(value: T) -> Self {
        MlspData {
            atomic_count: AtomicUsize::new(1),
            value
        }
    }

    /// Increment the atomic counter for a given MlspData pointer
    fn increment(data: *const MlspData<T>) -> usize {
        unsafe {
            let old = (*data).atomic_count.fetch_add(1, Ordering::Release);
            atomic::fence(Ordering::Acquire);
            old
        }
    }

    /// Decrement the atomic counter for a given MlspData pointer
    fn decrement(data: *mut MlspData<T>) {
        let counter = unsafe { &(*data).atomic_count };

        let old = counter.fetch_sub(1, Ordering::Release);
        atomic::fence(Ordering::Acquire);

        if old == 1 {
            unsafe {
                // drop the wrapped value
                ptr::drop_in_place(data);
            }
        }
    }

    /// Get a reference to the inner value
    fn get(&self) -> &T {
        &self.value
    }
}

/// Multi-Level Smart Pointer
/// 
/// A hybrid between Rc and Arc that attempts to reduce the number
/// of atomic operations performed when it is shared, cloned and dropped
/// within a thread.
/// 
/// Mlsp cannot be sent between threads.
/// ```compile_fail
/// use std::thread;
/// let a = mlsp::Mlsp::new(1u8);
/// thread::spawn(move || {
///     let a2 = a;
/// });
/// ```
/// 
/// To send across thread boundaries, first package using the `package()` method
/// and send the resulting package.
/// ```
/// use std::thread;
/// let a = mlsp::Mlsp::new(1u8);
/// let a_pkg = a.package();
/// thread::spawn(move || {
///     let a2 = a_pkg.unpackage();
/// });
/// ```
pub struct Mlsp<T> {
    local_count: *mut Cell<usize>,
    data_ptr: *mut MlspData<T>
}

impl<T> Mlsp<T> {
    /// Creates a new Mlsp wrapping the given value with local and atomic
    /// counts both equal to one
    pub fn new(value: T) -> Self {
        Mlsp {
            local_count: Box::into_raw(Box::new(Cell::new(1))),
            data_ptr: Box::into_raw(Box::new(MlspData::new(value)))
        }
    }

    /// Create a Send-able package from the Mlsp
    /// This increments the atomic_count
    pub fn package(&self) -> MlspPackage<T> {
        MlspData::increment(self.data_ptr);

        MlspPackage {
            data_ptr: self.data_ptr
        }
    }
}

impl<T> Borrow<T> for Mlsp<T> {
    fn borrow(&self) -> &T {
        unsafe {
            (*self.data_ptr).get()
        }
    }
}

impl<T> AsRef<T> for Mlsp<T> {
    fn as_ref(&self) -> &T {
        unsafe {
            (*self.data_ptr).get()
        }
    }
}

impl<T> Clone for Mlsp<T> {
    fn clone(&self) -> Self {
        unsafe {
            let count = (*self.local_count).get();
            let count = count + 1;
            (*self.local_count).set(count);
        }

        Mlsp {
            local_count: self.local_count,
            data_ptr: self.data_ptr
        }
    }
}

impl<T> Drop for Mlsp<T> {
    fn drop(&mut self) {
        // SAFETY: Performs unsafe operations on the counter pointer
        unsafe {
            // Decrement the local_count
            let count = (*self.local_count).get();
            let count = count - 1;
            (*self.local_count).set(count);

            // If the new value is greater than zero, there are still local references
            // and no further operations are needed.
            if count > 0 {
                return;
            }
        }
        // If the local_count was reduced to zero,
        // then this thread no longer has any references

        // 1. Drop the local counter being used by this thread
        // SAFETY: The only way to reach this line is by dropping all other references to self.local_count
        unsafe {
            ptr::drop_in_place(self.local_count);
        }
        // 2. Decrement the global pointer on the MlspData and drop if necessary
        MlspData::decrement(self.data_ptr);
    }
}

/// A reference to the contents of an Mlsp
/// that does not yet have a local counter and can be sent across threads.
pub struct MlspPackage<T> {
    data_ptr: *mut MlspData<T>
}

impl<T> MlspPackage<T> {
    /// Turns this package into a normal Mlsp that can
    /// be shared within this thread without atomic operations.
    pub fn unpackage(self) -> Mlsp<T> {
        Mlsp {
            local_count: Box::into_raw(Box::new(Cell::new(1))),
            data_ptr: self.data_ptr
        }
    }
}

impl<T> Drop for MlspPackage<T> {
    fn drop(&mut self) {
        // Decrement the global pointer on the MlspData and drop if necessary
        MlspData::decrement(self.data_ptr);
    }
}

unsafe impl<T> Send for MlspPackage<T> {}

impl<T> Clone for MlspPackage<T> {
    fn clone(&self) -> Self {
        MlspData::increment(self.data_ptr);

        MlspPackage {
            data_ptr: self.data_ptr
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_sharing() {
        let a = Mlsp::new(1u8);
        let b = a.clone();
        let c = b.clone();

        let d = c.package();
        let _d2 = d.unpackage();

        let e = c.package();
        let _e2 = e.unpackage();
    }

    #[test]
    fn cross_thread_sharing() {
        use std::thread;

        let mlsp = Mlsp::new(1u8);

        let mut children = vec![];

        for _ in 0..10 {
            let package = mlsp.package();

            children.push(thread::spawn(move || {
                let shared_mlsp = package.unpackage();
                let shared_mlsp_clone = shared_mlsp.clone();

                assert_eq!(1u8, *(shared_mlsp.borrow()));
                assert_eq!(1u8, *(shared_mlsp_clone.borrow()));
            }));
        }

        for child in children {
            // Wait for the thread to finish. Returns a result.
            let _ = child.join();
        }
    }
}
