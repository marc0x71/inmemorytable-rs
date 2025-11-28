use crate::{error::InMemoryTableError, internal::header::Header};

#[repr(u8)]
#[derive(PartialEq, Debug)]
enum SlotState {
    Free = 0,
    Occupied = 1,
}
#[repr(C)]
#[derive(PartialEq, Debug)]
struct SlotMetadata {
    state: SlotState,
    _padding: [u8; 7],
    size: usize, // if SlotState::Free it contains the next free record, otherwise it's the size of buffer
                 // after the "size" field there is the buffer
}

#[derive(Debug, Clone, Copy)]
struct SlotsHeader {
    /// maximum capacity
    capacity: usize,
    /// used records
    count: usize,
    /// record size
    slot_size: usize,
    /// first free node
    first_free: usize,
}
impl SlotsHeader {
    fn new(capacity: usize, slot_size: usize) -> Self {
        Self {
            capacity,
            count: 0,
            first_free: 0,
            slot_size,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Slots {
    header: Header<SlotsHeader>,
}

impl Slots {
    const SLOT_METADATA_SIZE: usize = std::mem::size_of::<SlotMetadata>();

    pub fn calculate_required_size(capacity: usize, data_size: usize) -> usize {
        let slot_size = Self::SLOT_METADATA_SIZE + data_size;
        slot_size * capacity
    }

    pub fn new(
        base_ptr: *mut u8,
        capacity: usize,
        data_size: usize,
    ) -> Result<Self, InMemoryTableError> {
        let slot_size = Self::SLOT_METADATA_SIZE + data_size;

        let header = Header::with_value(base_ptr, SlotsHeader::new(capacity, slot_size))?;
        let mut slots = Self { header };
        slots.initialize_free_list();
        Ok(slots)
    }

    pub fn open(base_ptr: *mut u8) -> Result<Self, InMemoryTableError> {
        let header = Header::new(base_ptr)?;
        Ok(Self { header })
    }

    pub fn initialize_free_list(&mut self) {
        let capacity = self.header.capacity;
        for i in 0..capacity {
            let slot = self.get_record_metadata_mut(i);
            slot.state = SlotState::Free;
            // for SlotState::Free the 'size' field indicate the next free slot
            slot.size = if i + 1 < capacity { i + 1 } else { capacity };
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.header.count
    }

    pub(crate) fn insert(&mut self, data: &[u8]) -> Result<usize, InMemoryTableError> {
        if self.header.count >= self.header.capacity {
            return Err(InMemoryTableError::TableFull {
                current: self.header.count,
                capacity: self.header.capacity,
            });
        }

        if data.len() > self.header.slot_size - Self::SLOT_METADATA_SIZE {
            return Err(InMemoryTableError::RecordSizeTooBig {
                current: data.len(),
                max: self.header.slot_size - Self::SLOT_METADATA_SIZE,
            });
        }

        let index = self.header.first_free;
        let next_free = self.get_record_metadata(index).size;

        // update header
        self.header.first_free = next_free; // update header.first_free
        self.header.count += 1;

        // write record in memory
        self.set_record_data(index, data);

        Ok(index)
    }

    pub(crate) fn update(&mut self, index: usize, data: &[u8]) -> Result<(), InMemoryTableError> {
        if data.len() > self.header.slot_size - Self::SLOT_METADATA_SIZE {
            return Err(InMemoryTableError::RecordSizeTooBig {
                current: data.len(),
                max: self.header.slot_size - Self::SLOT_METADATA_SIZE,
            });
        }

        self.set_record_data(index, data);
        Ok(())
    }

    pub(crate) fn get(&self, index: usize) -> &[u8] {
        unsafe {
            let ptr = self.header.data().add(index * self.header.slot_size);
            let record = &*(ptr as *const SlotMetadata);
            if record.state == SlotState::Free {
                return &[];
            }
            let ptr_data = ptr.add(Self::SLOT_METADATA_SIZE);
            std::slice::from_raw_parts(ptr_data, record.size)
        }
    }

    pub(crate) fn remove(&mut self, index: usize) {
        let next_free = self.header.first_free;

        // set record as free
        self.get_record_metadata_mut(index).state = SlotState::Free;
        self.get_record_metadata_mut(index).size = next_free;

        // update header
        self.header.first_free = index;
        self.header.count -= 1;
    }

    fn get_record_metadata(&self, index: usize) -> &SlotMetadata {
        let ptr = unsafe { self.header.data().add(index * self.header.slot_size) };
        unsafe { &*(ptr as *const SlotMetadata) }
    }

    fn get_record_metadata_mut(&mut self, index: usize) -> &mut SlotMetadata {
        let ptr = unsafe { self.header.data().add(index * self.header.slot_size) };
        unsafe { &mut *(ptr as *mut SlotMetadata) }
    }

    fn set_record_data(&self, index: usize, data: &[u8]) {
        unsafe {
            let ptr = self.header.data().add(index * self.header.slot_size);
            let record = &mut *(ptr as *mut SlotMetadata);
            record.state = SlotState::Occupied;
            record.size = data.len();
            let ptr_data = ptr.add(Self::SLOT_METADATA_SIZE);
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr_data, data.len());
        };
    }

    pub fn is_free(&self, index: usize) -> bool {
        let metadata = self.get_record_metadata(index);
        metadata.state == SlotState::Free
    }

    #[allow(dead_code)]
    pub fn is_occupied(&self, index: usize) -> bool {
        let metadata = self.get_record_metadata(index);
        metadata.state == SlotState::Occupied
    }
}
