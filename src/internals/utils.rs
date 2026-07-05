use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
};

use rand::{
    RngExt,
    distr::{Distribution, StandardUniform},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CacheState<T, DATA> {
    Ready(T),
    NotReady(DATA),
}

pub fn random_value<T>() -> T
where
    StandardUniform: Distribution<T>,
{
    rand::rng().random()
}

pub fn find_value_inbetween<T: PartialOrd>(
    mut values: impl ExactSizeIterator<Item = T>,
    value: T,
) -> Option<(T, usize)> {
    let mut idx = 0;
    let mut lhs = values.next()?;
    for rhs in values {
        idx += 1;
        if value >= lhs && value <= rhs {
            return Some((lhs, idx));
        }
        lhs = rhs;
    }
    None
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ExactLengthBuffer<T> {
    inner: Vec<T>,
    len: usize,
}

impl<T> ExactLengthBuffer<T> {
    pub fn new(len: usize) -> Self {
        Self {
            inner: Vec::new(),
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn len_mut(&mut self) -> &mut usize {
        &mut self.len
    }

    pub fn store(&mut self, item: T) {
        self.inner.push(item);

        // Ensure buffer size
        if self.len < self.inner.len() {
            // Remove the oldest item
            self.inner.swap_remove(0);
        }
    }

    pub fn remove(&mut self, idx: usize) -> T {
        self.inner.swap_remove(idx)
    }

    pub fn inner(&self) -> &[T] {
        &self.inner
    }
}

pub fn path_to_number(path: &PathBuf) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}
