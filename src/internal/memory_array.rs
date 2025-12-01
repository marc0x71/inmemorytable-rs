use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    slice,
};

use crate::error::InMemoryTableError;

#[derive(Debug)]
pub(crate) struct MemoryArray<T> {
    ptr: *mut u8,
    length: usize,
    _phantom: PhantomData<T>,
}

impl<T> MemoryArray<T> {
    pub fn new(ptr: *mut u8, length: usize) -> Result<Self, InMemoryTableError> {
        let alignment = std::mem::align_of::<T>();
        if !(ptr as usize).is_multiple_of(alignment) {
            return Err(InMemoryTableError::MisalignedMemory {
                address: ptr as usize,
                required: alignment,
            });
        }
        Ok(Self {
            ptr,
            length,
            _phantom: PhantomData,
        })
    }
    pub fn as_slice(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.ptr as *const T, self.length) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut T, self.length) }
    }

    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.length
    }
}

impl<T> Deref for MemoryArray<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> DerefMut for MemoryArray<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
