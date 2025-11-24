#![allow(dead_code)]

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Offset {
    value: usize, // 0 = None
}

impl Offset {
    const NONE_VALUE: usize = 0;

    pub const fn none() -> Self {
        Self {
            value: Self::NONE_VALUE,
        }
    }

    pub const fn some(offset: usize) -> Self {
        Self { value: offset + 1 }
    }

    pub fn get(&self) -> Option<usize> {
        if self.value == Self::NONE_VALUE {
            None
        } else {
            Some(self.value - 1)
        }
    }

    pub fn set(&mut self, offset: Option<usize>) {
        self.value = offset.unwrap_or(Self::NONE_VALUE);
    }

    pub fn is_some(&self) -> bool {
        self.value != Self::NONE_VALUE
    }

    pub fn is_none(&self) -> bool {
        self.value == Self::NONE_VALUE
    }

    pub fn unwrap(&self) -> usize {
        self.get()
            .expect("called `Offset::unwrap()` on a `None` value")
    }

    pub fn unwrap_or(&self, default: usize) -> usize {
        self.get().unwrap_or(default)
    }
}

// Conversions
impl From<usize> for Offset {
    fn from(value: usize) -> Self {
        Self::some(value)
    }
}
impl From<Option<usize>> for Offset {
    fn from(opt: Option<usize>) -> Self {
        match opt {
            Some(v) => Self::some(v),
            None => Self::none(),
        }
    }
}

impl From<Offset> for Option<usize> {
    fn from(opt: Offset) -> Self {
        opt.get()
    }
}
