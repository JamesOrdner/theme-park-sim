use std::{num::NonZeroU32, ops::Deref};

#[derive(Clone, Copy)]
pub struct EntityId(NonZeroU32);

impl EntityId {
    #[track_caller]
    pub fn new(val: u32) -> Self {
        #[cfg(debug_assertions)]
        return Self(NonZeroU32::new(val).expect("EntityId may not be 0"));

        #[cfg(not(debug_assertions))]
        return Self(unsafe { NonZeroU32::new_unchecked(val) });
    }
}

impl From<NonZeroU32> for EntityId {
    fn from(val: NonZeroU32) -> Self {
        Self(val)
    }
}

impl Deref for EntityId {
    type Target = NonZeroU32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
