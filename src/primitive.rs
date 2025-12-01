#![allow(unused)]

use std::hash::{Hash, Hasher};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash)]
#[repr(u8)]
pub enum TypeTag {
    U8,
    U16,
    U32,
    U64,
    U128,
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
    Bool,
}

pub trait Primitive: Copy + Sized {
    fn type_tag() -> TypeTag;

    fn to_id(self) -> Id {
        Id::from_value(self)
    }
}

// Implementazione manuale solo per i tipi primitivi
impl Primitive for u8 {
    fn type_tag() -> TypeTag {
        TypeTag::U8
    }
}

impl Primitive for u16 {
    fn type_tag() -> TypeTag {
        TypeTag::U16
    }
}

impl Primitive for u32 {
    fn type_tag() -> TypeTag {
        TypeTag::U32
    }
}

impl Primitive for u64 {
    fn type_tag() -> TypeTag {
        TypeTag::U64
    }
}

impl Primitive for u128 {
    fn type_tag() -> TypeTag {
        TypeTag::U128
    }
}

impl Primitive for i8 {
    fn type_tag() -> TypeTag {
        TypeTag::I8
    }
}

impl Primitive for i16 {
    fn type_tag() -> TypeTag {
        TypeTag::I16
    }
}

impl Primitive for i32 {
    fn type_tag() -> TypeTag {
        TypeTag::I32
    }
}

impl Primitive for i64 {
    fn type_tag() -> TypeTag {
        TypeTag::I64
    }
}

impl Primitive for i128 {
    fn type_tag() -> TypeTag {
        TypeTag::I128
    }
}

impl Primitive for f32 {
    fn type_tag() -> TypeTag {
        TypeTag::F32
    }
}

impl Primitive for f64 {
    fn type_tag() -> TypeTag {
        TypeTag::F64
    }
}

impl Primitive for bool {
    fn type_tag() -> TypeTag {
        TypeTag::Bool
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(C)]
pub struct Id {
    data: [u8; 16],
    len: usize,
    type_tag: TypeTag,
}
impl Id {
    pub fn from_value<T: Primitive>(value: T) -> Self {
        let mut data = [0u8; 16];
        let size = std::mem::size_of::<T>();

        unsafe {
            let ptr = &value as *const T as *const u8;
            std::ptr::copy_nonoverlapping(ptr, data.as_mut_ptr(), size);
        }

        Self {
            data,
            len: size,
            type_tag: T::type_tag(),
        }
    }
}

impl From<u8> for Id {
    fn from(value: u8) -> Self {
        Self::from_value(value)
    }
}
impl From<u16> for Id {
    fn from(value: u16) -> Self {
        Self::from_value(value)
    }
}
impl From<u32> for Id {
    fn from(value: u32) -> Self {
        Self::from_value(value)
    }
}
impl From<u64> for Id {
    fn from(value: u64) -> Self {
        Self::from_value(value)
    }
}
impl From<u128> for Id {
    fn from(value: u128) -> Self {
        Self::from_value(value)
    }
}
impl From<i8> for Id {
    fn from(value: i8) -> Self {
        Self::from_value(value)
    }
}
impl From<i16> for Id {
    fn from(value: i16) -> Self {
        Self::from_value(value)
    }
}
impl From<i32> for Id {
    fn from(value: i32) -> Self {
        Self::from_value(value)
    }
}
impl From<i64> for Id {
    fn from(value: i64) -> Self {
        Self::from_value(value)
    }
}
impl From<i128> for Id {
    fn from(value: i128) -> Self {
        Self::from_value(value)
    }
}
impl From<f32> for Id {
    fn from(value: f32) -> Self {
        Self::from_value(value)
    }
}
impl From<f64> for Id {
    fn from(value: f64) -> Self {
        Self::from_value(value)
    }
}
impl From<bool> for Id {
    fn from(value: bool) -> Self {
        Self::from_value(value)
    }
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash del type tag per distinguere tipi diversi
        self.type_tag.hash(state);

        // Hash solo dei bytes rilevanti (non tutti i 16)
        self.data[..self.len].hash(state);
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Id {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match self.type_tag.cmp(&other.type_tag) {
            Ordering::Equal => {}  // Stesso tipo, continua
            other => return other, // Tipi diversi
        }

        match self.type_tag {
            TypeTag::U8 => {
                let a = u8::from_ne_bytes([self.data[0]]);
                let b = u8::from_ne_bytes([other.data[0]]);
                a.cmp(&b)
            }
            TypeTag::U16 => {
                let a = u16::from_ne_bytes(self.data[..2].try_into().unwrap());
                let b = u16::from_ne_bytes(other.data[..2].try_into().unwrap());
                a.cmp(&b)
            }
            TypeTag::U32 => {
                let a = u32::from_ne_bytes(self.data[..4].try_into().unwrap());
                let b = u32::from_ne_bytes(other.data[..4].try_into().unwrap());
                a.cmp(&b)
            }
            TypeTag::U64 => {
                let a = u64::from_ne_bytes(self.data[..8].try_into().unwrap());
                let b = u64::from_ne_bytes(other.data[..8].try_into().unwrap());
                a.cmp(&b)
            }
            TypeTag::U128 => {
                let a = u128::from_ne_bytes(self.data);
                let b = u128::from_ne_bytes(other.data);
                a.cmp(&b)
            }
            TypeTag::I8 => {
                let a = i8::from_ne_bytes([self.data[0]]);
                let b = i8::from_ne_bytes([other.data[0]]);
                a.cmp(&b)
            }
            TypeTag::I16 => {
                let a = i16::from_ne_bytes(self.data[..2].try_into().unwrap());
                let b = i16::from_ne_bytes(other.data[..2].try_into().unwrap());
                a.cmp(&b)
            }
            TypeTag::I32 => {
                let a = i32::from_ne_bytes(self.data[..4].try_into().unwrap());
                let b = i32::from_ne_bytes(other.data[..4].try_into().unwrap());
                a.cmp(&b)
            }
            TypeTag::I64 => {
                let a = i64::from_ne_bytes(self.data[..8].try_into().unwrap());
                let b = i64::from_ne_bytes(other.data[..8].try_into().unwrap());
                a.cmp(&b)
            }
            TypeTag::I128 => {
                let a = i128::from_ne_bytes(self.data);
                let b = i128::from_ne_bytes(other.data);
                a.cmp(&b)
            }
            TypeTag::F32 => {
                let a = f32::from_ne_bytes(self.data[..4].try_into().unwrap());
                let b = f32::from_ne_bytes(other.data[..4].try_into().unwrap());
                a.total_cmp(&b)
            }
            TypeTag::F64 => {
                let a = f64::from_ne_bytes(self.data[..8].try_into().unwrap());
                let b = f64::from_ne_bytes(other.data[..8].try_into().unwrap());
                a.total_cmp(&b)
            }
            TypeTag::Bool => {
                let a = self.data[0] != 0;
                let b = other.data[0] != 0;
                a.cmp(&b)
            }
        }
    }
}
