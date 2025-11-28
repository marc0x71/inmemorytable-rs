use std::{
    fmt::Debug,
    hash::{DefaultHasher, Hash, Hasher},
    marker::PhantomData,
    ptr,
};

use crate::{
    error::InMemoryTableError,
    internal::{
        block::Block,
        memory_array::MemoryArray,
        offset::Offset,
        semaphore::SemaphoreSet,
        slot::{Slots, SlotsIterator},
    },
};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Bucket {
    node_position: Offset,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Node<K> {
    key: K,
    position: usize,
    next: Offset,
}

#[derive(Debug)]
pub(crate) struct Index<K> {
    block: Block,
    buckets: MemoryArray<Bucket>,
    slots: Slots,
    total_buckets: usize,
    semaphores: SemaphoreSet,
    _phantom: PhantomData<K>,
}

impl<K> Index<K> {
    const BUCKETS_FACTOR: f64 = 0.75;

    pub(crate) fn new(name: &str, size: usize) -> Result<Self, InMemoryTableError> {
        let total_buckets = (size as f64 * Self::BUCKETS_FACTOR).ceil() as usize;
        let total_buckets_size = std::mem::size_of::<Bucket>() * total_buckets;
        let total_size = total_buckets_size
            + Slots::calculate_required_size(size, std::mem::size_of::<Node<K>>());

        let block_name = format!("{name}_pk");
        let block = Block::new(&block_name, total_size)?;

        // initialize buckets
        let buckets = MemoryArray::new(block.ptr(), total_buckets_size)?;

        // initialize slots
        let slots_ptr = unsafe { block.ptr().add(total_buckets_size) };
        let slots = Slots::new(slots_ptr, size, std::mem::size_of::<Node<K>>())?;

        // create semaphores (capacity + 1)
        let semaphores = SemaphoreSet::create(&block_name, 0)?;

        Ok(Self {
            block,
            buckets,
            slots,
            total_buckets,
            semaphores,
            _phantom: PhantomData,
        })
    }

    pub(crate) fn open(name: &str, size: usize) -> Result<Self, InMemoryTableError> {
        let total_buckets = (size as f64 * Self::BUCKETS_FACTOR).ceil() as usize;
        let total_buckets_size = std::mem::size_of::<Bucket>() * total_buckets;

        let block_name = format!("{name}_pk");
        let block = Block::open(&block_name)?;

        // initialize buckets
        let buckets = MemoryArray::new(block.ptr(), total_buckets_size)?;

        // initialize slots
        let slots_ptr = unsafe { block.ptr().add(total_buckets_size) };
        let slots = Slots::open(slots_ptr)?;

        // create semaphores (capacity + 1)
        let semaphores = SemaphoreSet::open(&block_name, 0)?;

        Ok(Self {
            block,
            buckets,
            slots,
            total_buckets,
            semaphores,
            _phantom: PhantomData,
        })
    }

    fn read_node(&self, position: usize) -> Node<K> {
        let slice = self.slots.get(position);
        unsafe { ptr::read(slice.as_ptr() as *const Node<K>) }
    }

    fn insert_node(&mut self, node: Node<K>) -> Result<usize, InMemoryTableError> {
        let slice = unsafe {
            std::slice::from_raw_parts(
                (&node as *const Node<K>) as *const u8,
                std::mem::size_of::<Node<K>>(),
            )
        };

        self.slots.insert(slice)
    }

    fn update_node(&mut self, index: usize, node: Node<K>) -> Result<(), InMemoryTableError> {
        let slice = unsafe {
            std::slice::from_raw_parts(
                (&node as *const Node<K>) as *const u8,
                std::mem::size_of::<Node<K>>(),
            )
        };
        self.slots.update(index, slice)
    }

    fn remove_node(&mut self, index: usize) {
        self.slots.remove(index);
    }

    pub fn destroy(self) -> Result<(), InMemoryTableError> {
        self.block.destroy()?;
        self.semaphores.destroy()?;

        Ok(())
    }

    pub fn iter(&self) -> IndexIterator<'_, K> {
        IndexIterator {
            inner: self.slots.iter(),
            _phantom: PhantomData,
        }
    }
}

impl<K: Hash + Eq + Clone> Index<K> {
    fn hash(&self, key: &K) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as usize % self.total_buckets
    }

    pub(crate) fn insert(&mut self, key: K, position: usize) -> Result<(), InMemoryTableError> {
        let _lock = self.semaphores.lock_table();

        let bucket_idx = self.hash(&key);
        if let Some(mut last_node_idx) = self.buckets[bucket_idx].node_position.get() {
            // bucket exists

            let mut last_node = self.read_node(last_node_idx);
            while let Some(next) = last_node.next.get() {
                last_node_idx = next;
                last_node = self.read_node(last_node_idx);
            }

            // create node
            let new_node = Node {
                key: key.clone(),
                position,
                next: Offset::none(),
            };
            let next_node = self.insert_node(new_node)?;

            // update last node
            last_node.next = next_node.into();
            self.update_node(last_node_idx, last_node)?;
        } else {
            // empty bucket

            // create node
            let node = Node {
                key,
                position,
                next: Offset::none(),
            };
            let next_node = self.insert_node(node)?;

            // create bucket
            self.buckets[bucket_idx].node_position = next_node.into();
        }
        Ok(())
    }

    fn search(&self, key: &K) -> Option<(usize, Node<K>)>
    where
        K: Debug,
    {
        let bucket_idx = self.hash(key);
        if let Some(mut last_node_idx) = self.buckets[bucket_idx].node_position.get() {
            // bucket exists
            let mut last_node = self.read_node(last_node_idx);
            while last_node.key != *key {
                match last_node.next.get() {
                    Some(next) => {
                        last_node_idx = next;
                        last_node = self.read_node(last_node_idx);
                    }
                    None => return None,
                };
            }
            return Some((last_node_idx, last_node));
        }
        None
    }

    pub(crate) fn get(&self, key: K) -> Option<usize>
    where
        K: Debug,
    {
        let _lock = self.semaphores.lock_table();
        self.search(&key).map(|(_, node)| node.position)
    }

    pub(crate) fn remove(&mut self, key: K) -> Result<(), InMemoryTableError>
    where
        K: Debug,
    {
        let _lock = self.semaphores.lock_table();
        if let Some((removing_idx, removing_node)) = self.search(&key) {
            let bucket_idx = self.hash(&key);
            let bucket = &mut self.buckets[bucket_idx];
            let mut last_node_idx = bucket.node_position.get().unwrap();
            if last_node_idx == removing_idx {
                // the node is the first in the bucket
                bucket.node_position = removing_node.next;
                // self.write_bucket(bucket_idx, bucket);
                self.remove_node(removing_idx);
                return Ok(());
            }
            let mut last_node = self.read_node(last_node_idx);
            loop {
                match last_node.next.get() {
                    Some(next) if next == removing_idx => {
                        last_node.next = removing_node.next;
                        self.update_node(last_node_idx, last_node)?;
                        self.remove_node(removing_idx);
                        break;
                    }
                    Some(next) => {
                        last_node_idx = next;
                        last_node = self.read_node(last_node_idx);
                    }
                    None => break,
                };
            }
        }
        Ok(())
    }
}

pub struct IndexIterator<'a, K> {
    inner: SlotsIterator<'a>,
    _phantom: PhantomData<K>,
}

impl<K> Iterator for IndexIterator<'_, K> {
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|slice| {
            let node = unsafe { ptr::read(slice.as_ptr() as *const Node<K>) };
            node.key
        })
    }
}
