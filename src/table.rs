use std::{fmt::Debug, marker::PhantomData};

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    error::InMemoryTableError,
    internal::{
        block::Block,
        header::Header,
        index::{Index, IndexIterator},
        semaphore::SemaphoreSet,
        slot::{Slots, SlotsIterator},
    },
    record::TableRecord,
};

#[derive(Debug, Clone, Copy)]
struct TableHeader {
    /// magic number
    magic: [u8; 6],
    /// maximum capacity
    capacity: usize,
}

impl TableHeader {
    const MAGIC: [u8; 6] = *b"INMEMT";

    fn new(capacity: usize) -> Self {
        Self {
            magic: Self::MAGIC,
            capacity,
        }
    }

    fn is_valid(&self) -> bool {
        self.magic == Self::MAGIC
    }
}
/// A fixed-capacity record table stored in POSIX shared memory.
///
/// `Table` allows multiple processes to concurrently share structured data.
/// Records are serialized to a fixed-size region and accessed by primary key.
///
/// Internally it uses:
/// - `shm_open` + `mmap` to create or attach shared memory segment
/// - System V IPC semaphores for synchronization across processes
///
/// A table is referenced by a unique name (string), which maps to:
/// - a `/dev/shm/<name>` shared memory segment
/// - a semaphore set identified by a derived IPC key
///
/// # Type Requirements
///
/// The generic parameter `T` must implement [`TableRecord`] to expose a
/// key used for indexing. For insertion, lookup, removal and
/// key-based mutation, the record primary key uniquely identifies entries.
///
/// # Serialization
///
/// When `T: Serialize + DeserializeOwned`, `Table` stores `T` in serialized
/// form inside shared memory. Record size is fixed at creation time.  
///
/// The default record size is 2 KiB.  
/// Use [`Table::create_with_size`] if your serialized record type is larger.
///
/// # Concurrency
///
/// - `insert`, `remove`, and `update_with_lock` perform IPC semaphore locking.
/// - `find`, `count`, and `capacity` are safe concurrent readers.
/// - Locks are process-wide: multiple processes opening the same table
///   respect the same synchronization primitives.
///
/// # Destroy Semantics
///
/// [`Table::destroy`] removes both:
/// - the shared memory segment, and
/// - the semaphore set.
///
/// Destroying a table makes it unusable for all processes.
///
/// # Examples
///
/// ## Creating a table and inserting records
/// ```
/// use inmemorytable::table::Table;
/// use inmemorytable::record::TableRecord;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
/// struct TestData {
///     number: i32,
///     value: f64,
/// }
///
/// impl inmemorytable::record::TableRecord for TestData {
///     type Key = i32;
///     fn key(&self) -> Self::Key {
///         self.number
///     }
/// }
///
/// fn random_name() -> String {
///     use rand::{Rng, distr::Alphanumeric};
///     let s: String = rand::thread_rng()
///         .sample_iter(&Alphanumeric)
///         .take(8)
///         .map(char::from)
///         .collect();
///     format!("table_{}", s)
/// }
///
/// let name = random_name();
///
/// let mut table = Table::<TestData>::create(&name, 10).unwrap();
///
/// table.insert(&TestData { number: 1, value: 10.0 }).unwrap();
/// table.insert(&TestData { number: 2, value: 20.0 }).unwrap();
///
/// assert_eq!(table.count(), 2);
/// assert_eq!(table.capacity(), 10);
///
/// // Lookup
/// let got = table.find(2).unwrap().unwrap();
/// assert_eq!(got.value, 20.0);
///
/// table.destroy().unwrap();
/// ```
#[derive(Debug)]
pub struct Table<T: TableRecord> {
    block: Block,
    header: Header<TableHeader>,
    /// slot
    slots: Slots,
    /// semaphore set
    semaphores: SemaphoreSet,
    /// index
    primary_keys: Index<T::Key>,
    /// phantom data
    _phantom: PhantomData<T>,
}

impl<T: TableRecord> Table<T> {
    const DEFAULT_RECORD_SIZE: usize = 2048;
    const TABLE_HEADER_SIZE: usize = std::mem::size_of::<TableHeader>();

    fn calculate_total_size(capacity: usize, record_size: usize) -> usize {
        Self::TABLE_HEADER_SIZE + Slots::calculate_required_size(capacity, record_size)
    }

    /// Creates a new shared memory table with a fixed record capacity.
    ///
    /// The table will be allocated in `/dev/shm/<name>` and initialized.
    /// If a table with the same name already exists, returns [`InMemoryTableError::ShmAlreadyExists`].
    ///
    /// Each record slot has the default fixed record size (currently 2 KiB),
    /// chosen to accommodate serialized `T`. Use [`Table::create_with_size`] to manually
    /// specify the size.
    ///
    /// # Arguments
    ///
    /// * `name` — unique identifier used to name shared memory + semaphore set
    /// * `capacity` — maximum number of records that can be stored
    ///
    /// # Errors
    ///
    /// - Shared memory creation errors (permissions, no space, too large, etc)
    /// - Semaphore allocation errors (limit reached, permission denied, etc)
    ///
    /// # Examples
    ///
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData { number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self) -> Self::Key { self.number }
    /// }
    ///
    /// let name = "demo_table_create";
    /// let table = Table::<TestData>::create(name, 10).unwrap();
    /// table.destroy().unwrap();
    /// ```    
    pub fn create(name: &str, capacity: usize) -> Result<Self, InMemoryTableError> {
        Self::create_with_size(name, capacity, Self::DEFAULT_RECORD_SIZE)
    }

    /// Creates a shared memory table with a custom per-record memory size.
    ///
    /// Used when serialized `T` exceeds the default size (2 KiB).
    ///
    /// # Arguments
    ///
    /// * `name` — identifier for shared memory + semaphore set
    /// * `capacity` — maximum number of records
    /// * `record_size` — number of bytes reserved per stored record
    ///
    /// # Errors
    ///
    /// - If the record size is too small to store serialized data
    /// - POSIX shared memory + semaphore creation errors
    ///
    /// # Examples
    ///
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct BigRecord { data: Vec<u8> }
    ///
    /// impl TableRecord for BigRecord {
    ///     type Key = u32;
    ///     fn key(&self) -> u32 { 0 }
    /// }
    ///
    /// let name = "demo_big";
    /// let table = Table::<BigRecord>::create_with_size(name, 4, 4096).unwrap();
    /// table.destroy().unwrap();
    /// ```
    pub fn create_with_size(
        name: &str,
        capacity: usize,
        record_size: usize,
    ) -> Result<Self, InMemoryTableError> {
        let total_size = Self::calculate_total_size(capacity, record_size);
        let block = Block::new(name, total_size)?;

        let ptr = block.ptr();

        let header = Header::with_value(ptr, TableHeader::new(capacity))?;

        let alignment = std::mem::align_of::<TableHeader>();
        if !(ptr as usize).is_multiple_of(alignment) {
            return Err(InMemoryTableError::MisalignedMemory {
                address: ptr as usize,
                required: alignment,
            });
        }

        // initialize slots
        let slots_ptr = header.data();
        let slots = Slots::new(slots_ptr, capacity, record_size)?;

        // create semaphores (capacity + 1)
        let semaphores = SemaphoreSet::create(name, capacity)?;

        let primary_keys = Index::new(name, capacity)?;

        let table = Self {
            block,
            header,
            slots,
            semaphores,
            primary_keys,
            _phantom: PhantomData,
        };

        Ok(table)
    }

    /// Opens an existing shared memory table from another process.
    ///
    /// Loads metadata and attaches to existing semaphores.  
    /// Does **not** create shared memory if not found.
    ///
    /// # Errors
    ///
    /// - [`InMemoryTableError::NotInitialized`]
    /// - Semaphore errors
    ///
    /// # Example
    ///
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData { number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self) -> i32 { self.number }
    /// }
    ///
    /// let name = "demo_open";
    /// {
    ///     let mut t = Table::<TestData>::create(name, 5).unwrap();
    ///     t.insert(&TestData{ number: 1, value: 42.0 }).unwrap();
    /// }
    ///
    /// // Simulates a second process:
    /// let t2 = Table::<TestData>::open(name).unwrap();
    /// assert_eq!(t2.find(1).unwrap().unwrap().value, 42.0);
    /// t2.destroy().unwrap();
    /// ```
    pub fn open(name: &str) -> Result<Self, InMemoryTableError> {
        let block = Block::open(name)?;

        let ptr = block.ptr();

        let header: Header<TableHeader> = Header::new(ptr)?;

        let alignment = std::mem::align_of::<TableHeader>();
        if !(ptr as usize).is_multiple_of(alignment) {
            return Err(InMemoryTableError::MisalignedMemory {
                address: ptr as usize,
                required: alignment,
            });
        }

        if !header.is_valid() {
            return Err(InMemoryTableError::NotInitialized);
        }

        let slots_ptr = header.data();
        let slots = Slots::open(slots_ptr)?;

        let semaphores = SemaphoreSet::open(name, header.capacity)?;

        let primary_keys = Index::open(name, header.capacity)?;

        Ok(Self {
            block,
            header,
            slots,
            semaphores,
            primary_keys,
            _phantom: PhantomData,
        })
    }

    /// Returns the number of currently stored records.
    ///
    /// Does not require table-wide locking.
    ///
    /// # Example
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData { number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self)->i32 { self.number }
    /// }
    ///
    /// let name = "demo_count";
    /// let mut table = Table::<TestData>::create(name, 3).unwrap();
    ///
    /// assert_eq!(table.count(), 0);
    /// table.insert(&TestData{ number: 1, value: 1.0 }).unwrap();
    /// assert_eq!(table.count(), 1);
    ///
    /// table.destroy().unwrap();
    /// ```
    pub fn count(&self) -> usize {
        self.slots.count()
    }

    /// Returns the maximum allowed number of records.
    ///
    /// Constant after creation.
    ///
    /// # Example
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData { number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self)->i32 { self.number }
    /// }
    ///
    /// let table = Table::<TestData>::create("demo_cap", 7).unwrap();
    /// assert_eq!(table.capacity(), 7);
    /// table.destroy().unwrap();
    /// ```
    pub fn capacity(&self) -> usize {
        self.header.capacity
    }

    /// Deletes the table permanently.
    ///
    /// - Unlinks shared memory segment
    /// - Removes semaphore set
    ///
    /// After destruction, the table can no longer be used from any process.
    ///
    /// # Safety
    ///
    /// This is a destructive operation affecting **all** processes.
    ///
    /// # Examples
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData { number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self)->i32 { self.number }
    /// }
    ///
    /// let name = "demo_destroy";
    /// let table = Table::<TestData>::create(name, 3).unwrap();
    /// table.destroy().unwrap();
    /// ```
    pub fn destroy(self) -> Result<(), InMemoryTableError> {
        self.block.destroy()?;
        self.semaphores.destroy()?;
        self.primary_keys.destroy()?;

        Ok(())
    }
}

impl<T: TableRecord> Table<T>
where
    T: Serialize + DeserializeOwned,
{
    /// Locks a single record for exclusive in-place modification.
    ///
    /// The function:
    /// 1. Locates the record by key
    /// 2. Locks only that record
    /// 3. Deserializes it
    /// 4. Passes a mutable reference to your closure
    /// 5. Re-serializes and stores the updated value
    ///
    /// # Returns
    ///
    /// - `Ok(Some(result))` if the record exists
    /// - `Ok(None)` if no record was found
    ///
    /// # Example
    ///
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData { number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self) -> i32 { self.number }
    /// }
    ///
    /// let name = "demo_update";
    /// let mut t = Table::<TestData>::create(name, 5).unwrap();
    ///
    /// t.insert(&TestData{ number: 1, value: 10.0 }).unwrap();
    ///
    /// t.update_with_lock(1, |rec| {
    ///     rec.value += 100.0;
    /// }).unwrap();
    ///
    /// assert_eq!(t.find(1).unwrap().unwrap().value, 110.0);
    ///
    /// t.destroy().unwrap();
    /// ```
    pub fn update_with_lock<F, R>(
        &mut self,
        key: <T as TableRecord>::Key,
        f: F,
    ) -> Result<Option<R>, InMemoryTableError>
    where
        <T as TableRecord>::Key: Debug,
        F: FnOnce(&mut T) -> R,
    {
        if let Some(index) = self.primary_keys.get(key) {
            let _locked = self.semaphores.lock_record(index)?;
            if let Some(mut record) = self.get(index)? {
                let result = f(&mut record);
                self.update(index, &record)?;
                return Ok(Some(result));
            }
        }
        Ok(None)
    }

    fn update(&mut self, index: usize, record: &T) -> Result<(), InMemoryTableError> {
        let serialized = bincode::serialize(record)
            .map_err(|e| InMemoryTableError::SerializationError(e.to_string()))?;

        self.slots.update(index, &serialized)
    }

    /// Inserts a new record.
    ///
    /// Performs a **table-wide mutex lock** via IPC semaphores.
    /// The call blocks until exclusive access is granted.
    ///
    /// # Behavior
    ///
    /// - If the key already exists => [`InMemoryTableError::RecordDuplicated`]
    /// - If table is full => [`InMemoryTableError::TableFull`]
    ///
    /// # Returns
    ///
    /// Index of the inserted record (0-based).
    ///
    /// # Example
    ///
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData { number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self) -> i32 { self.number }
    /// }
    ///
    /// let name = "demo_insert";
    /// let mut table = Table::<TestData>::create(name, 2).unwrap();
    ///
    /// table.insert(&TestData { number: 10, value: 3.14 }).unwrap();
    /// table.insert(&TestData { number: 11, value: 2.71 }).unwrap();
    ///
    /// assert_eq!(table.count(), 2);
    ///
    /// table.destroy().unwrap();
    /// ```
    pub fn insert(&mut self, record: &T) -> Result<usize, InMemoryTableError>
    where
        <T as TableRecord>::Key: Debug,
    {
        if self.primary_keys.get(record.key()).is_some() {
            return Err(InMemoryTableError::RecordDuplicated);
        }
        let _locked = self.semaphores.lock_table()?;

        let serialized = bincode::serialize(record)
            .map_err(|e| InMemoryTableError::SerializationError(e.to_string()))?;

        let index = self.slots.insert(&serialized)?;

        // update primary_keys
        self.primary_keys.insert(record.key(), index)?;

        Ok(index)
    }

    fn get(&self, index: usize) -> Result<Option<T>, InMemoryTableError> {
        if index >= self.header.capacity {
            return Err(InMemoryTableError::InvalidIndex {
                index,
                max: self.header.capacity,
            });
        }

        let data = self.slots.get(index);

        // deserialize record
        let record: T = bincode::deserialize(data)
            .map_err(|e| InMemoryTableError::DeserializationError(e.to_string()))?;

        Ok(Some(record))
    }

    /// Looks up a record by primary key.
    ///
    /// Returns `Ok(Some(record))` if found, or `Ok(None)` if not present.
    /// Does not block the entire table.
    ///
    /// # Example
    ///
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData{ number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self) -> i32 { self.number }
    /// }
    ///
    /// let name = "demo_find";
    /// let mut t = Table::<TestData>::create(name, 3).unwrap();
    /// t.insert(&TestData{ number: 1, value: 10.0 }).unwrap();
    ///
    /// let got = t.find(1).unwrap();
    /// assert_eq!(got.unwrap().value, 10.0);
    ///
    /// assert!(t.find(999).unwrap().is_none());
    ///
    /// t.destroy().unwrap();
    /// ```
    pub fn find(&self, key: <T as TableRecord>::Key) -> Result<Option<T>, InMemoryTableError>
    where
        <T as TableRecord>::Key: Debug,
    {
        match self.primary_keys.get(key) {
            Some(index) => self.get(index),
            None => Ok(None),
        }
    }

    /// Deletes a record by primary key.
    ///
    /// Performs a table-wide lock.  
    /// Does **nothing** if the key is not present.
    ///
    /// # Example
    ///
    /// ```
    /// # use inmemorytable::table::Table;
    /// # use inmemorytable::record::TableRecord;
    /// # use serde::{Serialize, Deserialize};
    /// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    /// struct TestData{ number: i32, value: f64 }
    /// impl TableRecord for TestData {
    ///     type Key = i32;
    ///     fn key(&self) -> i32 { self.number }
    /// }
    ///
    /// let name = "demo_remove";
    /// let mut t = Table::<TestData>::create(name, 3).unwrap();
    /// t.insert(&TestData{ number: 1, value: 10.0 }).unwrap();
    ///
    /// t.remove(1).unwrap();
    /// assert!(t.find(1).unwrap().is_none());
    ///
    /// t.destroy().unwrap();
    /// ```
    pub fn remove(&mut self, key: <T as TableRecord>::Key) -> Result<(), InMemoryTableError>
    where
        <T as TableRecord>::Key: Debug,
    {
        if let Some(index) = self.primary_keys.get(key) {
            let _locked = self.semaphores.lock_record(index)?;

            self.slots.remove(index);
            return self.primary_keys.remove(key);
        }
        Ok(())
    }

    pub fn iter(&self) -> TableIterator<'_, T> {
        TableIterator {
            inner: self.slots.iter(),
            _phantom: PhantomData,
        }
    }

    pub fn keys(&self) -> IndexIterator<'_, <T as TableRecord>::Key> where {
        self.primary_keys.iter()
    }
}

pub struct TableIterator<'a, T: TableRecord> {
    inner: SlotsIterator<'a>,
    _phantom: PhantomData<T>,
}

impl<T> Iterator for TableIterator<'_, T>
where
    T: TableRecord + DeserializeOwned,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let data = self.inner.next()?;
        bincode::deserialize::<T>(data).ok()
    }
}
