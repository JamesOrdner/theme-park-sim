use std::{
    mem,
    ops::{Deref, DerefMut},
};

#[derive(Default)]
pub struct SystemSwapData<T> {
    data: T,
    swapped: bool,
}

impl<T> SystemSwapData<T> {
    pub fn swap(&mut self, other: &mut SystemSwapData<T>) {
        mem::swap(&mut self.data, &mut other.data);
        self.swapped = true;
        other.swapped = true;
    }

    /// Returns Some the first time this is called after swap()
    pub fn swapped(&mut self) -> Option<&mut T> {
        if self.swapped {
            self.swapped = false;
            Some(&mut self.data)
        } else {
            None
        }
    }
}

impl<T> Deref for SystemSwapData<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for SystemSwapData<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
