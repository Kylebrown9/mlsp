use core::cell::Cell;
use core::ptr;
use core::sync::atomic;
use core::marker::PhantomData;

use std::boxed::Box;
use std::mem;
use std::sync::atomic::AtomicUsize;

use std::sync::atomic::Ordering;

pub struct Mlsp<T: ?Sized> {
    local_count: *mut usize,
    atomic_count: *mut AtomicUsize,
    
    ptr: *mut T,

    phantom: PhantomData<T>
}

pub struct MlspPackage<T: ?Sized> {
    atomic_count: *mut AtomicUsize,
    
    ptr: *mut T,

    phantom: PhantomData<T>
}

impl<T> Mlsp<T> {
    pub fn new(value: T) -> Self {
        let local_count_cell = Cell::new(0);
        let atomic_count_cell = Cell::new(AtomicUsize::new(0));

        let output = Mlsp {
            local_count: local_count_cell.as_ptr(),
            atomic_count: atomic_count_cell.as_ptr(),

            ptr: Box::into_raw(Box::new(value)),

            phantom: PhantomData
        };

        mem::forget(local_count_cell);
        mem::forget(atomic_count_cell);

        output
    }

    pub fn package(&self) -> MlspPackage<T> {
        unsafe {
            (*self.atomic_count).fetch_add(1, Ordering::Release);
        }

        atomic::fence(Ordering::Acquire);

        MlspPackage {
            atomic_count: self.atomic_count,

            ptr: self.ptr,

            phantom: PhantomData
        }
    }
}

impl<T> MlspPackage<T> {
    pub fn unpackage(self) -> Mlsp<T> {
        let local_count_cell = Cell::new(0);

        let output = Mlsp {
            local_count: local_count_cell.as_ptr(),
            atomic_count: self.atomic_count,

            ptr: self.ptr,

            phantom: PhantomData
        };

        mem::forget(local_count_cell);

        output
    }
}

impl<T: ?Sized> Drop for Mlsp<T> {
    fn drop(&mut self) {
        unsafe {
            *self.local_count -= 1;

            if *self.local_count != 0 {
                return;
            }

            let last_atomic_count = (*self.atomic_count).fetch_sub(1, Ordering::Release);

            atomic::fence(Ordering::Acquire);

            if last_atomic_count == 1 {
                // destroy the contained object
                ptr::drop_in_place(self.ptr);
            }
        }
    }
}

impl<T: ?Sized> Clone for Mlsp<T> {
    fn clone(&self) -> Self {
        unsafe {
            *self.local_count += 1;
        }

        let output = Mlsp {
            local_count: self.local_count,
            atomic_count: self.atomic_count,

            ptr: self.ptr,

            phantom: PhantomData
        };

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    enum DropMock {
        
    }

    #[test]
    fn local_sharing() {
        let a = Mlsp::new(1u8);
        let b = a.clone();
    }
}
