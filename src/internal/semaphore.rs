use crate::error::InMemoryTableError;

fn sem_reset(sem_id: i32, size: usize) -> Result<(), InMemoryTableError> {
    for position in 0..size {
        let mut sembuf = libc::sembuf {
            sem_num: position as u16,
            sem_op: 1, // reset
            sem_flg: 0,
        };

        unsafe {
            if libc::semop(sem_id, &mut sembuf, 1) < 0 {
                // cleanup
                libc::semctl(sem_id, 0, libc::IPC_RMID);
                let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                return Err(InMemoryTableError::from_semop_error(errno, position, size));
            }
        }
    }
    Ok(())
}

pub struct Locker {
    sem_id: i32,
    position: usize,
}

impl Locker {
    fn try_lock(sem_id: i32, position: usize, size: usize) -> Result<Self, InMemoryTableError> {
        let mut sembuf = libc::sembuf {
            sem_num: position as u16,
            sem_op: -1, // lock
            sem_flg: libc::SEM_UNDO as i16,
        };

        unsafe {
            if libc::semop(sem_id, &mut sembuf, 1) < 0 {
                let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                return Err(InMemoryTableError::from_semop_error(errno, position, size));
            }
        }
        Ok(Self { sem_id, position })
    }
}

impl Drop for Locker {
    fn drop(&mut self) {
        let mut sembuf = libc::sembuf {
            sem_num: self.position as u16,
            sem_op: 1, // unlock
            sem_flg: libc::SEM_UNDO as i16,
        };

        unsafe {
            if libc::semop(self.sem_id, &mut sembuf, 1) < 0 {
                eprintln!(
                    "Unexpected error: semop unlock failed, position: {}, sem_id {}: {}",
                    self.position,
                    self.sem_id,
                    std::io::Error::last_os_error()
                );
            }
        }
    }
}

#[derive(Debug)]
pub struct SemaphoreSet {
    sem_id: i32,
    stripe_size: usize,
    stripes: usize,
    num_records: usize,
}

impl SemaphoreSet {
    const STRIPE_SIZE: usize = 1024;

    /// Generate key
    fn generate_sem_key(name: &str) -> i32 {
        let mut hash: i32 = 0x1234; // Seed iniziale

        for byte in name.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as i32);
        }

        let key = hash.abs();
        if key == 0 { 0x1234 } else { key }
    }

    /// Create a set of semaphores
    pub fn create(name: &str, num_records: usize) -> Result<Self, InMemoryTableError> {
        let stripe_size = if num_records < Self::STRIPE_SIZE {
            1
        } else {
            Self::STRIPE_SIZE
        };
        let stripes = num_records.div_ceil(stripe_size) + 1;
        let key = Self::generate_sem_key(name);

        unsafe {
            // IPC_CREAT | IPC_EXCL | 0666
            let sem_id = libc::semget(
                key,
                stripes as i32,
                libc::IPC_CREAT | libc::IPC_EXCL | 0o666,
            );

            if sem_id < 0 {
                let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                return Err(InMemoryTableError::from_semget_error(
                    errno, key, name, stripes,
                ));
            }

            // reset semaphores
            sem_reset(sem_id, stripes)?;

            Ok(Self {
                sem_id,
                stripe_size,
                stripes,
                num_records,
            })
        }
    }

    /// Open a set of existing semaphores
    pub fn open(name: &str, num_records: usize) -> Result<Self, InMemoryTableError> {
        let stripe_size = if num_records < Self::STRIPE_SIZE {
            1
        } else {
            Self::STRIPE_SIZE
        };
        let stripes = num_records.div_ceil(stripe_size) + 1;
        let key = Self::generate_sem_key(name);

        unsafe {
            let sem_id = libc::semget(key, stripes as i32, 0o666);

            if sem_id < 0 {
                let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                return Err(InMemoryTableError::from_semget_error(
                    errno, key, name, stripes,
                ));
            }

            Ok(Self {
                sem_id,
                stripe_size,
                stripes,
                num_records,
            })
        }
    }

    fn get_index(&self, position: usize) -> usize {
        position / self.stripe_size
    }

    /// Lock a single position
    fn lock(&self, position: usize) -> Result<Locker, InMemoryTableError> {
        let sem_num = self.get_index(position);
        if sem_num > self.stripes {
            return Err(InMemoryTableError::SemaphoreInvalidIndex {
                sem_num,
                max: self.stripes,
            });
        }
        Locker::try_lock(self.sem_id, self.get_index(position), self.stripes)
    }

    /// Lock entire semaphore set
    pub fn lock_table(&self) -> Result<Locker, InMemoryTableError> {
        Locker::try_lock(self.sem_id, self.num_records, self.stripes)
    }

    /// Lock a single record
    pub fn lock_record(&self, index: usize) -> Result<Locker, InMemoryTableError> {
        // I semafori 1..n sono per i singoli record
        self.lock(index)
    }

    /// Destroy the semaphore set
    pub fn destroy(&self) -> Result<(), InMemoryTableError> {
        unsafe {
            if libc::semctl(self.sem_id, 0, libc::IPC_RMID) < 0 {
                eprintln!("{}", std::io::Error::last_os_error());
                return Err(InMemoryTableError::SemaphoreError(format!(
                    "semctl IPC_RMID failed: {}",
                    std::io::Error::last_os_error()
                )));
            }
        }
        Ok(())
    }
}
