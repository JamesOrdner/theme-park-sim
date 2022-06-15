use std::{
    fmt::Display,
    iter::zip,
    num::NonZeroU32,
    ops::{Deref, Index, IndexMut, RangeBounds},
    slice,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(NonZeroU32);

impl EntityId {
    #[track_caller]
    pub fn new(val: u32) -> Self {
        #[cfg(debug_assertions)]
        return Self(NonZeroU32::new(val).expect("EntityId may not be 0"));

        #[cfg(not(debug_assertions))]
        return Self(unsafe { NonZeroU32::new_unchecked(val) });
    }

    pub fn min() -> Self {
        EntityId(NonZeroU32::new(1).unwrap())
    }

    pub fn max() -> Self {
        EntityId(NonZeroU32::new(u32::MAX).unwrap())
    }

    pub fn increment(&mut self) {
        self.0 = NonZeroU32::new(self.0.get() + 1).unwrap();
    }

    pub fn decrement(&mut self) {
        self.0 = NonZeroU32::new(self.0.get() - 1).unwrap();
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

impl Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct EntityMap<T> {
    entity_ids: Vec<EntityId>,
    data: Vec<T>,
}

impl<T> Default for EntityMap<T> {
    fn default() -> Self {
        Self {
            entity_ids: Vec::new(),
            data: Vec::new(),
        }
    }
}

impl<T> EntityMap<T> {
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    fn index(&self, entity_id: EntityId) -> Option<usize> {
        self.entity_ids
            .iter()
            .enumerate()
            .find(|(_, id)| **id == entity_id)
            .map(|a| a.0)
    }

    #[inline]
    pub fn insert(&mut self, entity_id: EntityId, data: T) {
        self.entity_ids.push(entity_id);
        self.data.push(data);
    }

    #[inline]
    pub fn remove(&mut self, entity_id: EntityId) -> T {
        let index = self.index(entity_id).unwrap();
        self.entity_ids.swap_remove(index);
        self.data.swap_remove(index)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.entity_ids.clear();
        self.data.clear();
    }

    #[inline]
    pub fn get(&self, entity_id: EntityId) -> Option<&T> {
        self.index(entity_id).and_then(|index| self.data.get(index))
    }

    #[inline]
    pub fn get_mut(&mut self, entity_id: EntityId) -> Option<&mut T> {
        self.index(entity_id)
            .and_then(|index| self.data.get_mut(index))
    }

    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    #[inline]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.data.iter_mut()
    }

    #[inline]
    pub fn iter(&self) -> Iter<T> {
        Iter {
            entity_id: self.entity_ids.iter(),
            data: self.data.iter(),
        }
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            entity_id: self.entity_ids.iter(),
            data: self.data.iter_mut(),
        }
    }

    #[inline]
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (EntityId, T)>,
    {
        let iter = iter.into_iter();

        let min_bound = iter.size_hint().0;
        self.entity_ids.reserve(min_bound);
        self.data.reserve(min_bound);

        for entry in iter {
            self.entity_ids.push(entry.0);
            self.data.push(entry.1);
        }
    }

    #[inline]
    pub fn drain<R>(&mut self, range: R) -> impl Iterator<Item = (EntityId, T)> + '_
    where
        R: RangeBounds<usize> + Copy,
    {
        zip(self.entity_ids.drain(range), self.data.drain(range))
    }
}

impl<T> Index<EntityId> for EntityMap<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: EntityId) -> &Self::Output {
        let index = self.index(index).unwrap();
        &self.data[index]
    }
}

impl<T> IndexMut<EntityId> for EntityMap<T> {
    #[inline]
    fn index_mut(&mut self, index: EntityId) -> &mut Self::Output {
        let index = self.index(index).unwrap();
        &mut self.data[index]
    }
}

pub struct Iter<'a, T> {
    entity_id: slice::Iter<'a, EntityId>,
    data: slice::Iter<'a, T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (&'a EntityId, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entity_id) = self.entity_id.next() {
            Some((entity_id, self.data.next().unwrap()))
        } else {
            None
        }
    }
}

impl<'a, T> IntoIterator for &'a EntityMap<T> {
    type Item = (&'a EntityId, &'a T);
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

pub struct IterMut<'a, T> {
    entity_id: slice::Iter<'a, EntityId>,
    data: slice::IterMut<'a, T>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (&'a EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entity_id) = self.entity_id.next() {
            Some((entity_id, self.data.next().unwrap()))
        } else {
            None
        }
    }
}

impl<'a, T> IntoIterator for &'a mut EntityMap<T> {
    type Item = (&'a EntityId, &'a mut T);
    type IntoIter = IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}
