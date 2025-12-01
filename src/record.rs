use std::hash::Hash;

use crate::index::IndexDef;

/// A trait for records that can be stored in a shared memory table.
///
/// This trait must be implemented for any type that needs to be stored in
/// a shared memory table structure. It defines how to extract a primary key
/// from the record
///
/// # Type Parameters
///
/// * `Key` - The type used as the primary key for this record. Must be:
///   * `Eq` - Comparable for equality
///   * `Hash` - Hashable for use in hash-based collections
///   * `Copy` - Efficiently copyable without heap allocation
///   * `Ord` - Totally ordered for sorting operations
///   * `Default` - Provides a default value
///
/// # Requirements
///
/// Types implementing this trait should also implement:
/// * `Serialize` and `Deserialize` from `serde` - Required for binary serialization
///   using `bincode` when storing/loading data from shared memory
///
/// # Examples
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use inmemorytable::record::TableRecord;
///
/// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
/// struct TestData {
///     number: i32,
///     value: f64,
/// }
///
/// impl TableRecord for TestData {
///     type Key = i32;
///
///     fn key(&self) -> Self::Key {
///         self.number
///     }
///
///     fn indexes() -> Vec<inmemorytable::index::IndexDef<Self>> {
///         vec![]
///     }
/// }
///
/// // Usage
/// let record = TestData { number: 42, value: 3.14 };
/// assert_eq!(record.key(), 42);
/// ```
///
/// # Another Example: Composite Key
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use inmemorytable::record::TableRecord;
///
/// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
/// struct User {
///     id: u64,
///     name: String,
///     email: String,
/// }
///
/// impl TableRecord for User {
///     type Key = u64;
///
///     fn key(&self) -> Self::Key {
///         self.id
///     }
///
///     fn indexes() -> Vec<inmemorytable::index::IndexDef<Self>> {
///         vec![]
///     }
/// }
/// ```
pub trait TableRecord: Sized + Clone {
    /// The type of the primary key used to identify this record.
    ///
    /// This type will be used for indexing, lookups in the shared memory table.
    type Key: Eq + Hash + Copy + Ord + Default;

    /// Returns the primary key for this record.
    ///
    /// This method extracts the key value that uniquely identifies this record
    /// in the table. The key is used for all lookup, insertion, and deletion
    /// operations.
    ///
    /// # Returns
    ///
    /// The primary key value of type `Self::Key`.
    fn key(&self) -> Self::Key;

    fn indexes() -> Vec<IndexDef<Self>>;
}
