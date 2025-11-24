use std::{
    ffi::{CString, c_void},
    os::fd::{AsRawFd, OwnedFd},
    ptr,
};

use nix::{
    sys::{
        mman::{shm_open, shm_unlink},
        stat::{Mode, fstat},
    },
    unistd::{close, ftruncate},
};

use crate::error::InMemoryTableError;

fn mmap_open(fd: &OwnedFd, size: usize) -> Result<*mut c_void, InMemoryTableError> {
    // mmap
    let ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        )
    };

    if ptr == libc::MAP_FAILED {
        let _ = close(fd.as_raw_fd());
        return Err(InMemoryTableError::ShmMappingFailed);
    }
    Ok(ptr)
}

#[derive(Debug)]
pub struct Block {
    /// table name
    name: CString,
    /// shm memory address
    ptr: *mut u8,
    /// memory size
    size: usize,
    /// File descriptor
    fd: Option<OwnedFd>,
}

impl Block {
    pub(crate) fn ptr(&self) -> *mut u8 {
        self.ptr
    }

    #[allow(dead_code)]
    pub(crate) fn size(&self) -> usize {
        self.size
    }

    pub(crate) fn new(name: &str, total_size: usize) -> Result<Self, InMemoryTableError> {
        // create shared memory
        let shm_name = CString::new(name)
            .map_err(|_| InMemoryTableError::InvalidName { name: name.into() })?;

        let fd = shm_open(
            shm_name.as_c_str(),
            nix::fcntl::OFlag::O_CREAT | nix::fcntl::OFlag::O_RDWR | nix::fcntl::OFlag::O_EXCL,
            Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IWGRP,
        )
        .map_err(|e| InMemoryTableError::ShmError(format!("shm_open failed: {}", e)))?;

        // set shared memory size
        ftruncate(&fd, total_size as i64).map_err(|e| {
            let _ = close(fd.as_raw_fd());
            let _ = shm_unlink(shm_name.as_c_str());
            InMemoryTableError::from_ftruncate_error(e, total_size)
        })?;

        // mmap
        let ptr = mmap_open(&fd, total_size).inspect_err(|_| {
            let _ = shm_unlink(shm_name.as_c_str());
        })?;

        // fill with 0
        unsafe {
            std::ptr::write_bytes(ptr as *mut u8, 0, total_size);
        }

        Ok(Self {
            name: shm_name,
            ptr: ptr as *mut u8,
            size: total_size,
            fd: Some(fd),
        })
    }

    pub(crate) fn open(name: &str) -> Result<Self, InMemoryTableError> {
        let shm_name = CString::new(name)
            .map_err(|_| InMemoryTableError::InvalidName { name: name.into() })?;

        let fd = shm_open(
            shm_name.as_c_str(),
            nix::fcntl::OFlag::O_RDWR,
            Mode::empty(),
        )
        .map_err(|e| InMemoryTableError::from_shm_error(e, name))?;

        let stat =
            fstat(&fd).map_err(|e| InMemoryTableError::ShmError(format!("fstat failed: {}", e)))?;

        let total_size = stat.st_size as usize;

        // mmap
        let ptr = mmap_open(&fd, total_size)?;

        Ok(Self {
            name: shm_name,
            ptr: ptr as *mut u8,
            size: total_size,
            fd: Some(fd),
        })
    }

    pub fn destroy(mut self) -> Result<(), InMemoryTableError> {
        // close file descriptor
        if let Some(fd) = self.fd.take() {
            drop(fd);
        }

        // remove shared memory
        shm_unlink(self.name.as_c_str())
            .map_err(|e| InMemoryTableError::ShmError(format!("shm_unlink failed: {}", e)))?;

        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn read<T>(&self, position: usize) -> T {
        unsafe {
            let record_ptr = self.ptr.add(position * std::mem::size_of::<T>()) as *const T;
            ptr::read(record_ptr)
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write<T>(&self, position: usize, record: T) {
        unsafe {
            let record_ptr = self.ptr.add(position * std::mem::size_of::<T>()) as *mut T;
            ptr::write(record_ptr, record)
        }
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        // memory unmap
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.size);
        }
    }
}
