use core::cell::Cell;
use core::ptr;
use core::sync::atomic;
use core::marker::PhantomData;

use std::boxed::Box;
use std::sync::atomic::AtomicUsize;

use std::sync::atomic::Ordering;

struct Packet<T> {
    atomic_count: AtomicUsize,
    value: T
}

pub struct Mlsp<T> {
    phantom: PhantomData<T>,
    local_count: *mut Cell<usize>,
    packet_ptr: *mut Packet<T>
}

pub struct MlspPackage<T> {
    phantom: PhantomData<T>,
    packet_ptr: *mut Packet<T>
}

impl<T> Packet<T> {
    fn new(value: T) -> Self {
        Packet {
            atomic_count: AtomicUsize::new(1),
            value: value
        }
    }

    fn increment(packet: *const Packet<T>) -> usize {
        unsafe {
            let old = (*packet).atomic_count.fetch_add(1, Ordering::Release);
            atomic::fence(Ordering::Acquire);
            old
        }
    }

    fn decrement(packet: *const Packet<T>) -> usize {
        unsafe {
            let old = (*packet).atomic_count.fetch_sub(1, Ordering::Release);
            atomic::fence(Ordering::Acquire);
            old
        }
    }

    fn get(&self) -> &T {
        &self.value
    }
}

impl<T> Mlsp<T> {
    pub fn new(value: T) -> Self {
        Mlsp {
            phantom: PhantomData,
            local_count: Box::into_raw(Box::new(Cell::new(1))),
            packet_ptr: Box::into_raw(Box::new(Packet::new(value)))
        }
    }

    pub fn get(&self) -> &T {
        unsafe {
            (*self.packet_ptr).get()
        }
    }

    pub fn package(&self) -> MlspPackage<T> {
        Packet::increment(self.packet_ptr);

        MlspPackage {
            phantom: PhantomData,
            packet_ptr: self.packet_ptr
        }
    }
}

impl<T> MlspPackage<T> {
    pub fn unpackage(self) -> Mlsp<T> {
        Mlsp {
            phantom: PhantomData,
            local_count: Box::into_raw(Box::new(Cell::new(1))),
            packet_ptr: self.packet_ptr
        }
    }
}

impl<T> Drop for Mlsp<T> {
    fn drop(&mut self) {
        unsafe {
            // Decrement the local_count
            let count = (*self.local_count).get();
            let count = count - 1;
            (*self.local_count).set(count);

            // If the new value is greater than zero the drop is complete
            if count > 0 {
                return;
            }

            // If the local_count was reduced to zero, then the atomic_count must be decremented
            let last_atomic_count = Packet::decrement(self.packet_ptr);

            if last_atomic_count == 1 {
                // drop the wrapped value
                ptr::drop_in_place(self.packet_ptr);
            }
        }
    }
}

impl<T> Drop for MlspPackage<T> {
    fn drop(&mut self) {
        unsafe {
            let last_atomic_count = Packet::decrement(self.packet_ptr);

            atomic::fence(Ordering::Acquire);

            if last_atomic_count == 1 {
                // drop the wrapped value
                ptr::drop_in_place(self.packet_ptr);
            }
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
            phantom: PhantomData,
            local_count: self.local_count,
            packet_ptr: self.packet_ptr
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
}
