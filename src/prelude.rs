extern crate alloc;

pub const MAX_FUTURE_BLOCK_TIME: i64 = 60 * 60 * 2;
pub const MEDIAN_TIME_PAST: usize = 11;
pub trait Median<T> {
    fn median(&mut self) -> Option<T>;
}

impl Median<i64> for Vec<i64> {
    fn median(&mut self) -> Option<i64> {
        self.sort();
        let len = self.len();
        if len == 0 {
            None
        } else if len % 2 == 1 {
            Some(self[len / 2])
        } else {
            let mid = len / 2;
            Some((self[mid - 1] + self[mid]) / 2)
        }
    }
}

impl Median<u64> for Vec<u64> {
    fn median(&mut self) -> Option<u64> {
        self.sort();
        let len = self.len();
        if len == 0 {
            None
        } else if len % 2 == 1 {
            Some(self[len / 2])
        } else {
            let mid = len / 2;
            Some((self[mid - 1] + self[mid]) / 2)
        }
    }
}

impl Median<u32> for Vec<u32> {
    fn median(&mut self) -> Option<u32> {
        self.sort();
        let len = self.len();
        if len == 0 {
            None
        } else if len % 2 == 1 {
            Some(self[len / 2])
        } else {
            let mid = len / 2;
            Some((self[mid - 1] + self[mid]) / 2)
        }
    }
}

#[derive(Debug)]
pub struct LRUCache<T: Send + Sync> {
    cache: Vec<T>,
    max_size: usize,
}

impl<T: Copy + Send + Sync> LRUCache<T> {
    pub(crate) fn new(max_size: usize) -> Self {
        LRUCache {
            cache: Vec::new(),
            max_size,
        }
    }

    pub(crate) fn add(&mut self, item: T)
    where
        T: Copy + Send + Sync,
    {
        self.cache.push(item);
        if self.cache.len() > self.max_size {
            self.cache.remove(0);
        }
    }
}

impl Median<u64> for LRUCache<u64> {
    fn median(&mut self) -> Option<u64> {
        self.cache.median()
    }
}

impl Median<i64> for LRUCache<i64> {
    fn median(&mut self) -> Option<i64> {
        self.cache.median()
    }
}

pub(crate) trait Stack<T, E>
where
    T: Sync + Send,
{
    async fn add(&mut self, item: T) -> Result<(), E>;
    fn root(&self) -> &T;
    fn tip(&self) -> &T;
}
