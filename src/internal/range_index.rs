#![allow(unused)]

use std::marker::PhantomData;

use crate::{
    error::InMemoryTableError,
    internal::{
        block::Block, header::Header, memory_array::MemoryArray, offset::Offset,
        semaphore::SemaphoreSet, slot::Slots,
    },
    primitive::Id,
};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RangeIndexHeader {
    /// maximum capacity
    capacity: usize,
    /// used records
    count: usize,
}

impl RangeIndexHeader {
    fn new(capacity: usize) -> Self {
        Self { capacity, count: 0 }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct RangeIndexEntry {
    id: Id,
    position: Offset,
}

#[derive(Debug)]
pub(crate) struct RangeIndex<T> {
    block: Block,
    header: Header<RangeIndexHeader>,
    entries: MemoryArray<RangeIndexEntry>,
    semaphores: SemaphoreSet,
    _phantom: PhantomData<T>,
}

impl<T> RangeIndex<T> {
    const RANGE_INDEX_HEADER_SIZE: usize = std::mem::size_of::<RangeIndexHeader>();

    pub(crate) fn new(name: &str, capacity: usize) -> Result<Self, InMemoryTableError> {
        let size = std::mem::size_of::<RangeIndexEntry>() * capacity;
        let total_size = size + Self::RANGE_INDEX_HEADER_SIZE;
        let block = Block::new(name, total_size)?;

        let ptr = block.ptr();
        let header = Header::with_value(ptr, RangeIndexHeader::new(capacity))?;

        // initialize entries
        let entries = MemoryArray::new(header.data(), capacity)?;

        // create semaphores (1)
        let semaphores = SemaphoreSet::create(name, 0)?;

        Ok(Self {
            block,
            header,
            entries,
            semaphores,
            _phantom: PhantomData,
        })
    }

    pub(crate) fn open(name: &str) -> Result<Self, InMemoryTableError> {
        let block = Block::open(name)?;

        let ptr = block.ptr();
        let header: Header<RangeIndexHeader> = Header::new(ptr)?;

        // initialize entries
        let entries = MemoryArray::new(header.data(), header.capacity)?;

        // create semaphores (1)
        let semaphores = SemaphoreSet::open(name, 0)?;

        Ok(Self {
            block,
            header,
            entries,
            semaphores,
            _phantom: PhantomData,
        })
    }

    pub fn destroy(mut self) -> Result<(), InMemoryTableError> {
        self.block.destroy()?;
        self.semaphores.destroy()?;

        Ok(())
    }

    fn find_position(&self, value: Id) -> usize {
        let slice = &self.entries[..self.header.count];

        match slice.binary_search_by_key(&value, |entry| entry.id) {
            Ok(pos) => pos,  // exact position where the id has been found
            Err(pos) => pos, // position where insert the new id
        }
    }

    pub(crate) fn insert(&mut self, value: Id, position: usize) -> Result<(), InMemoryTableError> {
        let pos = self.find_position(value);
        // Shift to right
        for i in (pos..self.header.count).rev() {
            self.entries[i + 1] = self.entries[i];
        }
        self.entries[pos] = RangeIndexEntry {
            id: value,
            position: position.into(),
        };
        self.header.count += 1;
        println!(
            "insert: {} [{:?}]",
            self.header.count,
            &self.entries.as_slice()
        );
        Ok(())
    }

    pub(crate) fn delete(&mut self, value: Id, position: usize) -> Result<(), InMemoryTableError> {
        let pos = self.find_position(value);

        // Search position of (value, slot)
        let mut found = None;
        for i in pos..self.header.count {
            if self.entries[i].id != value {
                // the value position has been exceeded
                break;
            }
            if self.entries[i].position == position.into() {
                found = Some(i);
                break;
            }
        }

        // remove
        if let Some(idx) = found {
            for i in idx..self.header.count - 1 {
                self.entries[i] = self.entries[i + 1];
            }
            self.header.count -= 1;
        }
        println!(
            "delete: {:?} [{:?}]",
            self.header.count,
            &self.entries.as_slice()
        );

        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        old_value: Id,
        new_value: Id,
        position: usize,
    ) -> Result<(), InMemoryTableError> {
        self.delete(old_value, position)?;
        self.insert(new_value, position)?;
        println!(
            "update: {} [{:?}]",
            self.header.count,
            &self.entries.as_slice()
        );
        Ok(())
    }
}
