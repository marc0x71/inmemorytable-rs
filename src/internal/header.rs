use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::error::InMemoryTableError;

#[derive(Debug)]
pub(crate) struct Header<T> {
    base_ptr: *mut u8,
    _phantom: PhantomData<T>,
}

impl<T> Header<T> {
    pub fn new(base_ptr: *mut u8) -> Result<Self, InMemoryTableError> {
        let alignment = std::mem::align_of::<T>();
        if !(base_ptr as usize).is_multiple_of(alignment) {
            return Err(InMemoryTableError::MisalignedMemory {
                address: base_ptr as usize,
                required: alignment,
            });
        }
        Ok(Self {
            base_ptr,
            _phantom: PhantomData,
        })
    }

    pub fn with_value(base_ptr: *mut u8, value: T) -> Result<Self, InMemoryTableError> {
        let alignment = std::mem::align_of::<T>();
        if !(base_ptr as usize).is_multiple_of(alignment) {
            return Err(InMemoryTableError::MisalignedMemory {
                address: base_ptr as usize,
                required: alignment,
            });
        }

        unsafe {
            let header_ptr = base_ptr as *mut T;
            ptr::write(header_ptr, value);
        };

        Ok(Self {
            base_ptr,
            _phantom: PhantomData,
        })
    }

    pub fn data(&self) -> *mut u8 {
        unsafe { self.base_ptr.add(std::mem::size_of::<T>()) }
    }
}

impl<T> Deref for Header<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.base_ptr as *const T) }
    }
}

impl<T> DerefMut for Header<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.base_ptr as *mut T) }
    }
}
