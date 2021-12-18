use core::cell::Cell;
use core::ptr;
use core::sync::atomic;

use std::borrow::Borrow;
use std::boxed::Box;

use std::ptr::NonNull;
use std::sync::atomic::Ordering;

/// The inner Arc-like portion of the Mlsp
/// It is a wrapper tha bundles an atomic usize reference counter
/// with an arbitrary value
struct MlspInner<T> {
    atomic_count: atomic::AtomicUsize,
    data: T
}

impl<T> MlspInner<T> {
    /// Creates a new data bundle with an atomic counter with value 1
    fn new(data: T) -> Self {
        MlspInner {
            atomic_count: atomic::AtomicUsize::new(1),
            data
        }
    }

    /// Increment the atomic counter for a given MlspInner pointer
    /// 
    /// # Safety
    /// A caller to increment is obligated to later call decrement exactly once,
    /// in order to ensure that the memory it contains is not leaked.
    unsafe fn increment(&self) {
        self.atomic_count.fetch_add(1, Ordering::Release);
    }

    /// Decrement the atomic counter for a given MlspInner pointer
    /// 
    /// # Safety
    /// For each call to decrement there must have been exactly one
    /// prior call to increment to prevent premature freeing.
    unsafe fn decrement(&mut self) {

        let old = self.atomic_count.fetch_sub(1, Ordering::Release);
        atomic::fence(Ordering::Acquire);

        // If the value before decrementing was one,
        // this caller is the last reference holder and the inner data must be dropped.
        if old == 1 {
            ptr::drop_in_place(self);
        }
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
/// # use std::thread;
/// let a = mlsp::Mlsp::new(1u8);
/// thread::spawn(move || {
///     let a2 = a;
/// });
/// ```
/// 
/// To send across thread boundaries, first package using the `package()` method
/// and send the resulting package.
/// ```
/// # use std::thread;
/// let a = mlsp::Mlsp::new(1u8);
/// let a_pkg = a.package();
/// thread::spawn(move || {
///     let a2 = a_pkg.unpackage();
/// });
/// ```
pub struct Mlsp<T> {
    local_count: NonNull<Cell<usize>>,
    inner_ptr: NonNull<MlspInner<T>>
}

impl<T> Mlsp<T> {
    /// Creates a new Mlsp wrapping the given value with local and atomic
    /// counts both equal to one
    pub fn new(data: T) -> Self {
        let atomic_counter = Box::new(MlspInner::new(data));
        let atomic_counter = Box::into_raw(atomic_counter);
        let atomic_counter = NonNull::new(atomic_counter).unwrap();

        Mlsp {
            local_count: new_local_counter(),
            inner_ptr: atomic_counter
        }
    }

    /// Create a Send-able package from the Mlsp
    /// This increments the atomic_count
    pub fn package(&self) -> MlspPackage<T> {
        unsafe {
            self.inner_ptr.as_ref().increment();
        }

        MlspPackage {
            inner_ptr: self.inner_ptr
        }
    }
}

impl<T> Borrow<T> for Mlsp<T> {
    fn borrow(&self) -> &T {
        unsafe {
            &self.inner_ptr.as_ref().data
        }
    }
}

impl<T> AsRef<T> for Mlsp<T> {
    fn as_ref(&self) -> &T {
        unsafe {
            &self.inner_ptr.as_ref().data
        }
    }
}

impl<T> Clone for Mlsp<T> {
    fn clone(&self) -> Self {
        unsafe {
            let local_count = self.local_count.as_ref();

            let count = local_count.get();
            let count = count + 1;
            local_count.set(count);
        }

        Mlsp {
            local_count: self.local_count,
            inner_ptr: self.inner_ptr
        }
    }
}

impl<T> Drop for Mlsp<T> {
    fn drop(&mut self) {
        // SAFETY: Requires that two `Mlsp`s for the same inner data must never exist in different threads 
        unsafe {
            let local_count = self.local_count.as_mut();
            // Decrement the local_count
            let count = local_count.get();
            let count = count - 1;
            local_count.set(count);

            // If the new value is greater than zero, there are still local references
            // and no further operations are needed.
            if count > 0 {
                return;
            }
        }
        // If the local_count was reduced to zero,
        // then this thread no longer has any references

        // SAFETY: Requires that no other `Mlsp`s exist that reference the same local_count
        unsafe {
            // Drop the local counter being used by this thread
            ptr::drop_in_place(self.local_count.as_mut());
            // Decrement the global pointer on the MlspInner and drop the inner data if necessary
            self.inner_ptr.as_mut().decrement();
        }
    }
}

/// A reference to the contents of an Mlsp
/// that does not yet have a local counter and can be sent across threads.
pub struct MlspPackage<T> {
    inner_ptr: NonNull<MlspInner<T>>
}

impl<T> MlspPackage<T> {
    /// Turns this package into a normal Mlsp that can
    /// be shared within this thread without atomic operations.
    pub fn unpackage(self) -> Mlsp<T> {
        Mlsp {
            local_count: new_local_counter(),
            inner_ptr: self.inner_ptr
        }
    }
}

impl<T> Drop for MlspPackage<T> {
    fn drop(&mut self) {
        unsafe {
            // Decrement the global pointer on the MlspInner and drop if necessary
            self.inner_ptr.as_mut().decrement();
        }
    }
}

unsafe impl<T: Sync + Send> Send for MlspPackage<T> {}
unsafe impl<T: Sync + Send> Sync for MlspPackage<T> {}

impl<T> Clone for MlspPackage<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.inner_ptr.as_ref().increment();
        }

        MlspPackage {
            inner_ptr: self.inner_ptr
        }
    }
}


fn new_local_counter() -> NonNull<Cell<usize>> {
    // Allocate the counter as a boxed cell
    let local_counter: Box<Cell<usize>> = Box::new(Cell::new(1));
    // Create a mutable pointer to the cell and prevent dropping
    let local_counter: *mut Cell<usize> = Box::into_raw(local_counter);
    // Turn that pointer into a NonNull
    let local_counter: NonNull<Cell<usize>> = NonNull::new(local_counter).unwrap();

    local_counter
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

        // Convince clippy that we need these values
        drop(c);
        drop(b);
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
