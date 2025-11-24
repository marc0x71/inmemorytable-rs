# InMemoryTable

A Rust library for managing typed records in POSIX shared memory with IPC semaphore synchronization.

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

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
- 🎯 **Zero-Copy Reads**: Direct memory access where possible

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

### Custom Record Size

By default, each record is allocated 2KB. For larger records, use `create_with_size`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
struct LargeDocument {
    id: u64,
    content: String,
    metadata: Vec<u8>,
}

impl TableRecord for LargeDocument {
    type Key = u64;
    fn key(&self) -> Self::Key { self.id }
}

// Allocate 8KB per record
let table = Table::<LargeDocument>::create_with_size(
    "documents",
    100,    // capacity
    8192,   // 8KB per record
)?;
```

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
| `remove(key)` | Remove a record by key |
| `count()` | Get current number of records |
| `capacity()` | Get maximum capacity |
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
- **Linux** (uses POSIX shared memory and System V IPC semaphores)
- Other Unix-like systems may work but are untested

### Permissions
- Write access to `/dev/shm`
- Ability to create IPC semaphores

### System Resources

Check your system limits:

```bash
# Available shared memory space
df -h /dev/shm

# Current semaphore usage
ipcs -s

# System limits for IPC resources
ipcs -l
```

## Performance Considerations

- **Memory Allocation**: Tables allocate all memory upfront based on `capacity × record_size`
- **Default Record Size**: 2KB per record (configurable with `create_with_size`)
- **Serialization**: Uses `bincode` for efficient binary serialization
- **Lock Contention**: High concurrent access may cause contention on semaphores
- **No Dynamic Resizing**: Choose capacity carefully - tables cannot be resized after creation

## Debugging and Cleanup

### Manual Cleanup

If a process crashes before calling `destroy()`, resources may remain:

```bash
# List shared memory segments
ls -lh /dev/shm/

# Remove orphaned shared memory
rm /dev/shm/table_name

# List IPC semaphores
ipcs -s

# Remove semaphore by ID
ipcrm -s <semaphore_id>

# Remove all semaphores owned by user
ipcrm -a
```

### Diagnostic Tools

```bash
# Monitor shared memory usage
watch -n 1 'df -h /dev/shm'

# Watch semaphore creation/deletion
watch -n 1 'ipcs -s'

# Check for orphaned resources
ls /dev/shm/ | grep -v "^\..*"
```

## Thread Safety

- ✅ **Process-Safe**: Multiple processes can safely access the same table
- ✅ **Lock-Free Reads**: Reads use semaphores for consistency
- ✅ **Atomic Updates**: `update_with_lock()` ensures atomic read-modify-write
- ⚠️ **Not Thread-Safe Within Process**: Use external synchronization (e.g., `Mutex`) if sharing a `Table` instance across threads in the same process

## Roadmap

- [ ] Secondary indexes support
- [ ] Dynamic table resizing
- [ ] Transaction support (BEGIN/COMMIT/ROLLBACK)
- [ ] Windows support (named shared memory)
- [ ] Comprehensive benchmarks
- [ ] Iterator support
- [ ] Batch operations
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
