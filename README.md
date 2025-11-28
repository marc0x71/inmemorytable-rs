# InMemoryTable

A Rust library for managing typed records in POSIX shared memory with IPC semaphore synchronization.

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust CI](https://github.com/marc0x71/inmemorytable-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/marc0x71/inmemorytable-rs/actions/workflows/ci.yml)

## ⚠️ Development Status

**This library is currently under active development and is NOT production-ready.**

- API may change without notice
- Not all edge cases have been thoroughly tested
- Performance optimizations are ongoing
- **Use at your own risk in production environments**

We welcome feedback, bug reports, and contributions to help make this library production-ready!

## Features

- 🚀 **High-Performance IPC**: Leverages POSIX shared memory for fast inter-process communication
- 🔒 **Thread-Safe**: System V IPC semaphores ensure safe concurrent access across processes
- 📦 **Type-Safe**: Generic table implementation with compile-time type checking
- 🔑 **Primary Key Indexing**: Fast O(1) lookups using hash-based indexing
- 💾 **Binary Serialization**: Efficient storage using `bincode`
- 🔄 **Full CRUD Operations**: Create, Read, Update, Delete with atomic operations
- 📦 **Batch Updates**: Update multiple records in a single call via `update_many`
- 🎯 **Zero-Copy Reads**: Direct memory access where possible
- 🔃 **Iterator Support**: Iterate over all valid records or keys in the table

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
inmemorytable = "0.1.0"  # Replace with actual version
serde = { version = "1.0", features = ["derive"] }
```

## Quick Start

### 1. Define Your Record Type

```rust
use serde::{Deserialize, Serialize};
use inmemorytable::record::TableRecord;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct Product {
    id: u32,
    name: String,
    price: f64,
    stock: i32,
}

impl TableRecord for Product {
    type Key = u32;
    
    fn key(&self) -> Self::Key {
        self.id
    }
}
```

### 2. Create and Use a Table

```rust
use inmemorytable::table::Table;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new table in shared memory
    let mut table = Table::<Product>::create("products", 1000)?;
    
    // Insert a record
    let product = Product {
        id: 1,
        name: "Laptop".to_string(),
        price: 999.99,
        stock: 50,
    };
    table.insert(&product)?;
    
    // Find a record
    if let Some(found) = table.find(1)? {
        println!("Found: {} - ${}", found.name, found.price);
    }
    
    // Update with automatic locking
    table.update_with_lock(1, |p| {
        p.stock -= 1;
        p.price *= 0.9; // 10% discount
    })?;
    
    // Remove a record
    table.remove(1)?;
    
    // Clean up
    table.destroy()?;
    
    Ok(())
}
```

## Usage Examples

### Multi-Process Communication

**Process A (Producer)**:
```rust
use inmemorytable::table::Table;
use serde::{Deserialize, Serialize};
use inmemorytable::record::TableRecord;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Task {
    id: u32,
    description: String,
    completed: bool,
}

impl TableRecord for Task {
    type Key = u32;
    fn key(&self) -> Self::Key { self.id }
}

fn main() {
    let mut table = Table::<Task>::create("task_queue", 100)
        .expect("Failed to create table");
    
    for i in 0..10 {
        let task = Task {
            id: i,
            description: format!("Task {}", i),
            completed: false,
        };
        table.insert(&task).expect("Failed to insert");
    }
    
    println!("Created {} tasks", table.count());
}
```

**Process B (Consumer)**:
```rust
fn main() {
    let mut table = Table::<Task>::open("task_queue")
        .expect("Failed to open table");
    
    for i in 0..10 {
        table.update_with_lock(i, |task| {
            println!("Processing: {}", task.description);
            task.completed = true;
        }).expect("Failed to update");
    }
    
    println!("Processed {} tasks", table.count());
    table.destroy().expect("Failed to cleanup");
}
```

### Iterating Over Records

The library provides iterator support for traversing all valid records in a table. The iterator automatically skips deleted or empty slots:

```rust
use inmemorytable::table::Table;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut table = Table::<Product>::create("inventory", 100)?;
    
    // Insert some products
    for i in 0..5 {
        let product = Product {
            id: i,
            name: format!("Product {}", i),
            price: 10.0 * (i as f64 + 1.0),
            stock: 100,
        };
        table.insert(&product)?;
    }
    
    // Remove one product
    table.remove(2)?;
    
    // Iterate over remaining products
    println!("Current inventory:");
    for product in table.iter() {
        println!("  - {} (ID: {}): ${:.2}", product.name, product.id, product.price);
    }
    
    // Use iterator with standard Rust methods
    let total_value: f64 = table.iter()
        .map(|p| p.price * p.stock as f64)
        .sum();
    println!("Total inventory value: ${:.2}", total_value);
    
    // Count products over a certain price
    let expensive_count = table.iter()
        .filter(|p| p.price > 25.0)
        .count();
    println!("Products over $25: {}", expensive_count);
    
    // Collect into a Vec
    let all_products: Vec<Product> = table.iter().collect();
    println!("Total products: {}", all_products.len());
    
    table.destroy()?;
    Ok(())
}
```

#### Iterator Behavior

- **Skips empty slots**: The iterator only yields valid records, automatically skipping deleted entries
- **Fused iterator**: After returning `None`, subsequent calls to `next()` will continue to return `None`
- **Snapshot semantics**: The iterator traverses the table as it exists at iteration time
- **No ordering guarantee**: Records are returned in internal storage order, not by key

### Iterating Over Keys

For cases where you only need the keys (more lightweight than full record iteration), use `keys()`:

```rust
use inmemorytable::table::Table;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut table = Table::<Product>::create("inventory", 100)?;
    
    // Insert some products
    for i in 0..5 {
        let product = Product {
            id: i,
            name: format!("Product {}", i),
            price: 10.0 * (i as f64 + 1.0),
            stock: 100,
        };
        table.insert(&product)?;
    }
    
    // Remove one product
    table.remove(2)?;
    
    // Get all keys
    let keys: Vec<u32> = table.keys().collect();
    println!("Active product IDs: {:?}", keys); // [0, 1, 3, 4]
    
    // Useful pattern: iterate keys, then update selectively
    for key in table.keys() {
        if key % 2 == 0 {
            table.update_with_lock(key, |p| p.price *= 0.9)?; // 10% discount on even IDs
        }
    }
    
    // Check if a specific set of keys exists
    let required_ids = vec![0, 1, 4];
    let existing_keys: Vec<_> = table.keys().collect();
    let all_present = required_ids.iter().all(|id| existing_keys.contains(id));
    println!("All required products present: {}", all_present);
    
    table.destroy()?;
    Ok(())
}
```

#### Keys Iterator Behavior

- **Lightweight**: Only returns keys, no deserialization of full records
- **Same skip behavior**: Automatically skips deleted/empty slots
- **Useful for**: Selective updates, existence checks, key export

### Batch Updates

For updating multiple records in a single call, use `update_many`:

```rust
use inmemorytable::table::Table;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut table = Table::<Product>::create("inventory", 100)?;
    
    // Insert some products
    for i in 0..10 {
        let product = Product {
            id: i,
            name: format!("Product {}", i),
            price: 10.0 * (i as f64 + 1.0),
            stock: 100,
        };
        table.insert(&product)?;
    }
    
    // Update specific products by key
    let updated = table.update_many([1, 3, 5, 7], |p| {
        p.price *= 0.9;  // 10% discount
    })?;
    println!("Updated {} products", updated);
    
    // Non-existent keys are silently skipped
    let updated = table.update_many([1, 2, 999], |p| {
        p.stock += 50;
    })?;
    assert_eq!(updated, 2);  // only 1 and 2 exist, 999 is skipped
    
    // Combine with keys() for conditional updates
    let low_stock: Vec<_> = table.iter()
        .filter(|p| p.stock < 20)
        .map(|p| p.id)
        .collect();
    table.update_many(low_stock, |p| p.stock += 100)?;
    
    table.destroy()?;
    Ok(())
}
```

#### Why Use `update_many`

- **Cleaner API**: Express batch updates in a single call
- **Returns count**: Know how many records were actually updated
- **Skips missing keys**: Non-existent keys are silently ignored without errors

### Optimizing Record Size

By default, each record is allocated 2KB. For cache-like scenarios with many small records, use `create_with_size` to optimize memory usage:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
struct CacheEntry {
    id: u64,
    timestamp: i64,
    value: f64,
    flags: u8,
}

impl TableRecord for CacheEntry {
    type Key = u64;
    fn key(&self) -> Self::Key { self.id }
}

// This struct serializes to ~25 bytes, so 128 bytes is plenty
// Memory usage: 100,000 × 128 bytes = ~12 MB
// vs default:   100,000 × 2048 bytes = ~195 MB
let table = Table::<CacheEntry>::create_with_size(
    "price_cache",
    100_000,  // capacity
    128,      // 128 bytes per record
)?;
```

#### Choosing the Right Record Size

```rust
// Tip: Check your serialized size
let entry = CacheEntry { id: 1, timestamp: 0, value: 0.0, flags: 0 };
let serialized = bincode::serialize(&entry).unwrap();
println!("Serialized size: {} bytes", serialized.len());

// Add some padding for safety (20-50% overhead)
let recommended_size = serialized.len() + (serialized.len() / 4);
```

> ⚠️ **Note**: If a record exceeds the allocated size during serialization, the insert will fail. Always test with your largest expected record.

### Concurrent Updates from Threads

```rust
use std::thread;

let table_name = "shared_counter";

// Create table
{
    let mut table = Table::<Counter>::create(table_name, 10)?;
    table.insert(&Counter { id: 0, value: 0 })?;
}

// Spawn multiple threads
let handles: Vec<_> = (0..5)
    .map(|_| {
        let name = table_name.to_string();
        thread::spawn(move || {
            let mut table = Table::<Counter>::open(&name).unwrap();
            for _ in 0..100 {
                table.update_with_lock(0, |c| c.value += 1).unwrap();
            }
        })
    })
    .collect();

// Wait for completion
for handle in handles {
    handle.join().unwrap();
}

// Verify: should be 500 (5 threads × 100 increments)
let table = Table::<Counter>::open(table_name)?;
let counter = table.find(0)?.unwrap();
assert_eq!(counter.value, 500);
```

## API Overview

### Table Operations

| Method | Description |
|--------|-------------|
| `create(name, capacity)` | Create a new table in shared memory |
| `create_with_size(name, capacity, record_size)` | Create with custom record size |
| `open(name)` | Open an existing table from another process |
| `insert(record)` | Insert a new record (fails if key exists) |
| `find(key)` | Find a record by primary key |
| `update_with_lock(key, closure)` | Atomically update a record with locking |
| `update_many(keys, closure)` | Update multiple records by key in a single call |
| `remove(key)` | Remove a record by key |
| `count()` | Get current number of records |
| `capacity()` | Get maximum capacity |
| `iter()` | Get an iterator over all valid records |
| `keys()` | Get an iterator over all valid keys (lightweight) |
| `destroy()` | Remove table from shared memory |

### Error Handling

The library provides detailed error types through `InMemoryTableError`:

```rust
match table.insert(&record) {
    Ok(index) => println!("Inserted at index {}", index),
    Err(e) => match e {
        InMemoryTableError::TableFull { current, capacity } => {
            eprintln!("Table full: {}/{}", current, capacity);
        },
        InMemoryTableError::RecordDuplicated => {
            eprintln!("Record with this key already exists");
        },
        InMemoryTableError::ShmNoSpace => {
            eprintln!("Insufficient space on /dev/shm");
        },
        _ => eprintln!("Error: {}", e),
    }
}
```

## System Requirements

### Operating System
- **Linux**: Full support
- **macOS**: Supported (uses POSIX shared memory and System V IPC)
- Other Unix-like systems (FreeBSD, etc.) may work but are untested

### Permissions
- Write access to shared memory (Linux: `/dev/shm`, macOS: managed by kernel)
- Ability to create IPC semaphores

### System Resources

Check your system limits:

```bash
# Available shared memory space (Linux only)
df -h /dev/shm

# Current semaphore usage
ipcs -s

# System limits for IPC resources
ipcs -l
```

## Performance Considerations

- **Memory Allocation**: Tables allocate all memory upfront based on `capacity × record_size`
- **Default Record Size**: 2KB per record - consider reducing for cache scenarios with small records
- **Serialization**: Uses `bincode` for efficient binary serialization
- **Lock Contention**: High concurrent access may cause contention on semaphores
- **No Dynamic Resizing**: Choose capacity carefully - tables cannot be resized after creation
- **Iterator Performance**: Iteration stops as soon as all valid records are found - O(count) in best case (no deletions), O(capacity) in worst case (deletions created sparse slots). Use `keys()` when you don't need full records
- **Best Use Cases**: High-frequency IPC, shared caches, real-time data sharing between processes

## Debugging and Cleanup

### Manual Cleanup

If a process crashes before calling `destroy()`, resources may remain:

**Linux:**
```bash
# List shared memory segments
ls -lh /dev/shm/

# Remove orphaned shared memory
rm /dev/shm/table_name
```

**macOS:**
```bash
# List shared memory segments
ls -lh /var/folders/*/*/com.apple.shm/
```

**Both systems (System V IPC):**
```bash
# List IPC semaphores
ipcs -s

# Remove semaphore by ID
ipcrm -s <semaphore_id>

# Remove all semaphores owned by user (Linux only)
ipcrm -a
```

### Diagnostic Tools

```bash
# Monitor shared memory usage (Linux)
watch -n 1 'df -h /dev/shm'

# Watch semaphore creation/deletion
watch -n 1 'ipcs -s'

# Check for orphaned resources (Linux)
ls /dev/shm/ | grep -v "^\..*"
```

## Thread Safety

- ✅ **Process-Safe**: Multiple processes can safely access the same table
- ✅ **Lock-Free Reads**: Reads use semaphores for consistency
- ✅ **Atomic Updates**: `update_with_lock()` ensures atomic read-modify-write
- ⚠️ **Not Thread-Safe Within Process**: Use external synchronization (e.g., `Mutex`) if sharing a `Table` instance across threads in the same process
- ⚠️ **Iterator Safety**: Iterators are not synchronized - concurrent modifications during iteration may lead to inconsistent results

## Roadmap

- [ ] Secondary indexes support
- [ ] Dynamic table resizing
- [ ] Transaction support (BEGIN/COMMIT/ROLLBACK)
- [ ] Windows support (named shared memory)
- [ ] Comprehensive benchmarks
- [x] Iterator support
- [x] Keys iterator (`keys()`)
- [x] Batch update (`update_many`)
- [ ] Additional iterators (`drain()`)
- [ ] Conditional batch update (`update_where`) - awaiting secondary indexes
- [ ] Query builder API
- [ ] Backup/restore functionality

## Contributing

Contributions are welcome! This library is in active development and needs:

- 🐛 Bug reports and fixes
- 📝 Documentation improvements
- ✨ Feature implementations
- 🧪 More test cases
- 📊 Performance benchmarks

Please open an issue before starting major work to discuss your ideas.

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_concurrent_updates_with_threads

# Check for memory leaks (requires valgrind)
valgrind --leak-check=full cargo test
```

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Built with [serde](https://serde.rs/) for serialization
- Uses [bincode](https://github.com/bincode-org/bincode) for binary encoding
- Error handling with [thiserror](https://github.com/dtolnay/thiserror)
- System calls via [nix](https://github.com/nix-rust/nix)

## Support

- 📖 [Documentation](https://docs.rs/inmemorytable)
- 🐛 [Issue Tracker](https://github.com/marc0x71/inmemorytable-rs/issues)
- 💬 [Discussions](https://github.com/marc0x71/inmemorytable-rs/discussions)

---

**⚠️ Remember: This is alpha software. Test thoroughly before any production use!**
