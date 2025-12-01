use thiserror::Error;

#[derive(Error, Debug)]
pub enum InMemoryTableError {
    /// Invalid shared memory name (EINVAL or CString error)
    #[error("Invalid name: '{name}'")]
    InvalidName { name: String },

    // ============= Shared Memory Errors =============
    /// Shared memory with this name already exists (EEXIST)
    #[error("Shared memory '{name}' already exists. Run cleanup or use open().")]
    ShmAlreadyExists { name: String },

    /// Insufficient permissions to create shared memory (EACCES/EPERM)
    #[error("Insufficient permissions to create shared memory")]
    ShmPermissionDenied,

    /// Insufficient space on /dev/shm (ENOSPC)
    #[error("Insufficient space on /dev/shm. Check: df -h /dev/shm")]
    ShmNoSpace,

    /// Requested size too large (EFBIG)
    #[error("Requested size too large: {requested} bytes")]
    ShmTooLarge { requested: usize },

    /// Insufficient memory (ENOMEM)
    #[error("Insufficient memory for shared memory")]
    ShmOutOfMemory,

    /// Memory mapping error (mmap failed)
    #[error("Memory mapping error")]
    ShmMappingFailed,

    /// Invalid file descriptor (EBADF)
    #[error("Invalid file descriptor")]
    ShmBadFileDescriptor,

    /// Generic shared memory error (fallback)
    #[error("Shared memory error: {0}")]
    ShmError(String),

    // ============= Semaphore Errors =============
    /// IPC semaphore set with this key already exists (EEXIST)
    #[error(
        "IPC semaphore already exists\n  Key: {key} (0x{key:X})\n  Table: {name}\n  Cleanup: ipcrm -s $(ipcs -s | grep '{key:x}' | awk '{{print $2}}')"
    )]
    SemaphoreAlreadyExists { key: i32, name: String },

    /// System semaphore limit reached (ENOSPC)
    #[error("System semaphore limit reached. View: ipcs -s")]
    SemaphoreLimitReached,

    /// Insufficient permissions to create semaphores (EACCES/EPERM)
    #[error("Insufficient permissions to create semaphores")]
    SemaphorePermissionDenied,

    /// Invalid number of semaphores requested (EINVAL)
    #[error("Invalid number of semaphores: requested {requested} (system max: ~250)")]
    SemaphoreInvalidParams { requested: usize },

    /// Insufficient kernel memory for semaphores (ENOMEM)
    #[error("Insufficient kernel memory for semaphores")]
    SemaphoreOutOfMemory,

    /// Semaphore operation error (semop failed)
    #[error("Semaphore operation error (index: {sem_num})")]
    SemaphoreOperationFailed { sem_num: usize },

    /// Semaphore removed during operation (EIDRM)
    #[error("Semaphore removed by another process during operation")]
    SemaphoreRemoved,

    /// Invalid semaphore (index out of range)
    #[error("Invalid semaphore index: {sem_num} (max: {max})")]
    SemaphoreInvalidIndex { sem_num: usize, max: usize },

    /// Semaphore not exists
    #[error("Semaphore not exists: {key}")]
    SemaphoreNotExists { key: i32 },

    /// Generic semaphore error (fallback)
    #[error("Semaphore error: {0}")]
    SemaphoreError(String),

    // ============= Other Errors =============
    /// Table full - maximum capacity reached
    #[error("Table full: {current}/{capacity} records")]
    TableFull { current: usize, capacity: usize },

    /// Invalid index for record access
    #[error("Invalid index: {index} (available records: 0..{max})")]
    InvalidIndex { index: usize, max: usize },

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// Table not properly initialized
    #[error("Table not properly initialized (invalid magic number)")]
    NotInitialized,

    /// Record too big
    #[error("Record size too big: current {current}, max {max}")]
    RecordSizeTooBig { current: usize, max: usize },

    #[error("Record duplicated")]
    RecordDuplicated,

    #[error("Misaligned memory: address {address}, required {required}")]
    MisalignedMemory { address: usize, required: usize },

    #[error("PrimaryKey changed")]
    PrimaryKeyChanged,

    #[error("Index name too long: current {current}, max {max}")]
    IndexNameTooLong { current: usize, max: usize },

    #[error("Too many indexes: current {current}, max {max}")]
    TooManyIndexes { current: usize, max: usize },

    #[error("Index name '{name}' not found")]
    IndexNotFound { name: String },
}

impl InMemoryTableError {
    /// Convert POSIX errors from shm_open in InMemoryTableError
    pub(crate) fn from_shm_error(err: nix::Error, name: &str) -> Self {
        match err {
            nix::errno::Errno::EEXIST => Self::ShmAlreadyExists {
                name: name.to_string(),
            },
            nix::errno::Errno::EACCES | nix::errno::Errno::EPERM => Self::ShmPermissionDenied,
            nix::errno::Errno::EINVAL => Self::InvalidName {
                name: name.to_string(),
            },
            nix::errno::Errno::ENOSPC => Self::ShmNoSpace,
            nix::errno::Errno::ENOMEM => Self::ShmOutOfMemory,
            nix::errno::Errno::EBADF => Self::ShmBadFileDescriptor,
            _ => Self::ShmError(format!("{}", err)),
        }
    }

    /// Convert POSIX errors from ftruncate in InMemoryTableError
    pub(crate) fn from_ftruncate_error(err: nix::Error, size: usize) -> Self {
        match err {
            nix::errno::Errno::EFBIG => Self::ShmTooLarge { requested: size },
            nix::errno::Errno::ENOSPC => Self::ShmNoSpace,
            _ => Self::ShmError(format!("ftruncate: {}", err)),
        }
    }

    /// Convert POSIX errors from semget in InMemoryTableError
    pub(crate) fn from_semget_error(errno: i32, key: i32, name: &str, num_sems: usize) -> Self {
        match errno {
            libc::EEXIST => Self::SemaphoreAlreadyExists {
                key,
                name: name.to_string(),
            },
            libc::ENOSPC => Self::SemaphoreLimitReached,
            libc::EACCES | libc::EPERM => Self::SemaphorePermissionDenied,
            libc::EINVAL => Self::SemaphoreInvalidParams {
                requested: num_sems,
            },
            libc::ENOMEM => Self::SemaphoreOutOfMemory,
            libc::ENOENT => Self::SemaphoreNotExists { key },
            _ => Self::SemaphoreError(format!(
                "semget errno: {} ({})",
                errno,
                std::io::Error::from_raw_os_error(errno)
            )),
        }
    }

    /// Convert POSIX errors from semop in InMemoryTableError
    pub(crate) fn from_semop_error(errno: i32, sem_num: usize, max: usize) -> Self {
        match errno {
            libc::EIDRM => Self::SemaphoreRemoved,
            libc::EINVAL | libc::EFBIG => Self::SemaphoreInvalidIndex { sem_num, max },
            _ => Self::SemaphoreOperationFailed { sem_num },
        }
    }
}
