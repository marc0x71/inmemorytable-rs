#![allow(unused)]

use std::{collections::HashMap, fmt, marker::PhantomData};

use crate::{
    error::InMemoryTableError,
    internal::{
        block::Block,
        hash_index::HashIndex,
        memory_array::MemoryArray,
        range_index::{self, RangeIndex},
    },
    primitive::{Id, Primitive},
    record::TableRecord,
    table::Table,
};

#[derive(Debug, Copy, Clone)]
pub enum IndexKind {
    Hash,
    Range,
}
pub struct IndexDef<T> {
    pub name: String,
    pub kind: IndexKind,
    pub extractor: Box<dyn Fn(&T) -> Id>,
}
impl<T: TableRecord> std::fmt::Debug for IndexDef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexDef")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("extractor", &"<function>")
            .finish()
    }
}

static MAX_INDEXES: usize = 16;
static MAX_INDEX_NAME: usize = 31;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct IndexSlot {
    name: [char; MAX_INDEX_NAME],
    kind: IndexKind,
}
impl Default for IndexSlot {
    fn default() -> Self {
        Self {
            name: Default::default(),
            kind: IndexKind::Range,
        }
    }
}

impl<T> From<&IndexDef<T>> for IndexSlot {
    fn from(value: &IndexDef<T>) -> Self {
        let mut name: [char; MAX_INDEX_NAME] = ['\0'; MAX_INDEX_NAME];
        name[..value.name.chars().count()].copy_from_slice(&value.name.chars().collect::<Vec<_>>());
        IndexSlot {
            name,
            kind: value.kind,
        }
    }
}

#[derive(Debug)]
enum IndexType {
    Hash(HashIndex<Id>),
    Range(RangeIndex<Id>),
}

struct Index<T> {
    index_type: IndexType,
    extractor: Box<dyn Fn(&T) -> Id>,
}
impl<T: TableRecord> std::fmt::Debug for Index<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexDef")
            .field("type", &self.index_type)
            .field("extractor", &"<function>")
            .finish()
    }
}
impl<T> Index<T> {
    fn new(
        table_name: &str,
        definition: IndexDef<T>,
        size: usize,
    ) -> Result<Self, InMemoryTableError> {
        let index_name = format!("{table_name}_{}_idx", definition.name);
        let index = match definition.kind {
            IndexKind::Hash => IndexType::Hash(HashIndex::new(&index_name, size)?),
            IndexKind::Range => IndexType::Range(RangeIndex::new(&index_name, size)?),
        };
        Ok(Self {
            index_type: index,
            extractor: definition.extractor,
        })
    }
    fn insert(&mut self, record: &T, position: usize) -> Result<(), InMemoryTableError> {
        let value = (self.extractor)(record);
        match self.index_type {
            IndexType::Hash(ref mut hash_index) => hash_index.insert(value, position),
            IndexType::Range(ref mut range_index) => range_index.insert(value, position),
        }
    }
    fn update(
        &mut self,
        old_record: &T,
        new_record: &T,
        position: usize,
    ) -> Result<(), InMemoryTableError> {
        let old_value = (self.extractor)(old_record);
        let new_value = (self.extractor)(new_record);
        match self.index_type {
            IndexType::Hash(ref mut hash_index) => {
                hash_index.update(old_value, new_value, position)
            }
            IndexType::Range(ref mut range_index) => {
                range_index.update(old_value, new_value, position)
            }
        }
    }
    fn remove(&mut self, record: &T, position: usize) -> Result<(), InMemoryTableError> {
        let value = (self.extractor)(record);
        match self.index_type {
            IndexType::Hash(ref mut hash_index) => hash_index.delete(value, position),
            IndexType::Range(ref mut range_index) => range_index.delete(value, position),
        }
    }
    fn destroy(self) -> Result<(), InMemoryTableError> {
        match self.index_type {
            IndexType::Hash(hash_index) => hash_index.destroy(),
            IndexType::Range(range_index) => range_index.destroy(),
        }
    }
}

#[derive(Debug)]
pub struct Indexes<T: TableRecord> {
    table_name: String,
    capacity: usize,
    block: Block,
    buckets: MemoryArray<IndexSlot>,
    definitions: HashMap<String, Index<T>>,
}

impl<T: TableRecord> Indexes<T> {
    pub(crate) fn new(name: &str, capacity: usize) -> Result<Self, InMemoryTableError> {
        let block_name = format!("{name}_idx");
        let size = MAX_INDEXES * std::mem::size_of::<IndexSlot>();

        let block = Block::new(&block_name, size)?;
        let buckets = MemoryArray::new(block.ptr(), MAX_INDEXES)?;

        let mut indexes = Self {
            table_name: name.into(),
            capacity,
            block,
            buckets,
            definitions: HashMap::new(),
        };
        indexes.create_indexes(T::indexes())?;

        Ok(indexes)
    }

    pub(crate) fn open(name: &str, capacity: usize) -> Result<Self, InMemoryTableError> {
        let block_name = format!("{name}_idx");

        let block = Block::open(&block_name)?;
        let buckets = MemoryArray::new(block.ptr(), MAX_INDEXES)?;

        let mut indexes = Self {
            table_name: name.into(),
            capacity,
            block,
            buckets,
            definitions: HashMap::new(),
        };
        indexes.use_indexes(T::indexes())?;

        Ok(indexes)
    }

    pub fn destroy(mut self) -> Result<(), InMemoryTableError> {
        for (_, mut idx) in self.definitions.into_iter() {
            idx.destroy()?;
        }

        self.block.destroy()?;

        Ok(())
    }

    fn check_provided_indexes(&mut self, list: &[IndexDef<T>]) -> Result<(), InMemoryTableError> {
        if list.len() > MAX_INDEXES {
            return Err(InMemoryTableError::TooManyIndexes {
                current: list.len(),
                max: MAX_INDEXES,
            });
        }

        let max = list.iter().map(|d| d.name.len()).max().unwrap_or(0);
        if max > MAX_INDEX_NAME {
            return Err(InMemoryTableError::IndexNameTooLong {
                current: max,
                max: MAX_INDEX_NAME,
            });
        }
        Ok(())
    }

    fn create_indexes(&mut self, list: Vec<IndexDef<T>>) -> Result<(), InMemoryTableError> {
        if list.is_empty() {
            return Ok(());
        }
        self.check_provided_indexes(&list)?;

        // create in memory definition
        let mut definitions = list.iter().map(|d| d.into()).collect::<Vec<_>>();
        definitions.resize(MAX_INDEXES, IndexSlot::default());
        let mem = self.buckets.as_mut_slice();
        mem.copy_from_slice(&definitions);

        self.definitions = list
            .into_iter()
            .map(|d| {
                let name = d.name.clone();
                let idx = Index::new(self.table_name.as_str(), d, self.capacity)?;
                Ok((name, idx))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        Ok(())
    }

    fn use_indexes(&mut self, list: Vec<IndexDef<T>>) -> Result<(), InMemoryTableError> {
        if list.is_empty() {
            return Ok(());
        }
        self.check_provided_indexes(&list)?;
        // TODO verificare che gli indici definiti in SHM siano gli stessi della lista specificata in
        // TableRecord::indexes()

        self.definitions = list
            .into_iter()
            .map(|d| {
                let name = d.name.clone();
                let idx = Index::new(self.table_name.as_str(), d, self.capacity)?;
                Ok((name, idx))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        Ok(())
    }

    pub(crate) fn insert(&mut self, record: &T, position: usize) -> Result<(), InMemoryTableError> {
        for (_, mut idx) in self.definitions.iter_mut() {
            idx.insert(record, position)?
        }
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        old_record: T,
        new_record: T,
        position: usize,
    ) -> Result<(), InMemoryTableError> {
        for (_, mut idx) in self.definitions.iter_mut() {
            idx.update(&old_record, &new_record, position)?
        }
        Ok(())
    }

    pub(crate) fn remove(&mut self, record: &T, position: usize) -> Result<(), InMemoryTableError> {
        for (_, mut idx) in self.definitions.iter_mut() {
            idx.remove(record, position)?
        }
        Ok(())
    }

    pub fn query_range_index<'a, P: Fn(usize) -> Option<T>>(
        &'a self,
        index_name: &str,
        provider: P,
    ) -> Result<IndexQuery<RangeIndexRef<'a>, T, P>, InMemoryTableError> {
        self.definitions
            .get(index_name)
            .map(|idx| match &idx.index_type {
                IndexType::Range(index) => Ok(IndexQuery {
                    index_ref: RangeIndexRef(index),
                    provider,
                    _phantom: PhantomData,
                }),
                IndexType::Hash(_) => Err(InMemoryTableError::InvalidIndexType),
            })
            .ok_or(InMemoryTableError::IndexNotFound {
                name: index_name.to_string(),
            })?
    }

    pub fn query_hash_index<'a, P: Fn(usize) -> Option<T>>(
        &'a self,
        index_name: &str,
        provider: P,
    ) -> Result<IndexQuery<HashIndexRef<'a>, T, P>, InMemoryTableError> {
        self.definitions
            .get(index_name)
            .map(|idx| match &idx.index_type {
                IndexType::Hash(index) => Ok(IndexQuery {
                    index_ref: HashIndexRef(index),
                    provider,
                    _phantom: PhantomData,
                }),
                IndexType::Range(_) => Err(InMemoryTableError::InvalidIndexType),
            })
            .ok_or(InMemoryTableError::IndexNotFound {
                name: index_name.to_string(),
            })?
    }
}

pub struct HashIndexRef<'a>(&'a HashIndex<Id>);
pub struct RangeIndexRef<'a>(&'a RangeIndex<Id>);

pub struct IndexQuery<I, T: TableRecord, P: Fn(usize) -> Option<T>> {
    index_ref: I,
    provider: P,
    _phantom: PhantomData<T>,
}

impl<T: TableRecord, P: Fn(usize) -> Option<T>> IndexQuery<HashIndexRef<'_>, T, P> {
    pub fn eq<V: Into<Id>>(&self, value: V) -> Option<T> {
        None
    }
}

impl<T: TableRecord, P: Fn(usize) -> Option<T>> IndexQuery<RangeIndexRef<'_>, T, P> {
    pub fn eq<V: Into<Id>>(&self, value: V) -> Vec<T> {
        vec![]
    }
    pub fn gt<V: Into<Id>>(&self, value: V) -> Vec<T> {
        vec![]
    }
    pub fn lt<V: Into<Id>>(&self, value: V) -> Vec<T> {
        vec![]
    }
}
