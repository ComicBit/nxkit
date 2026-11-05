//! The block cache shared by all versions: a decrypted chunk of an NCA section is produced once (concurrent readers of the
//! same chunk wait for the first one) and kept until the capacity pushes it out (oldest first). Base blocks that both
//! `v0` and the update's relocated reads need are decrypted once as long as the two reads are close in time, which the
//! job order (both versions interleaved by path) arranges.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::error::{ExtractError, Result};

/// Chunk size in bytes (a multiple of the IVFC block size 0x4000 and of the AES block).
pub const CHUNK: u64 = 1 << 20;

type Slot = Arc<OnceLock<std::result::Result<Arc<Vec<u8>>, String>>>;

struct Inner {
    map: HashMap<(u32, u64), Slot>,
    order: VecDeque<(u32, u64)>,
}

/// Counters for the summary.
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheStats {
    /// Chunks produced (decrypted).
    pub produced: u64,
    /// Bytes produced.
    pub produced_bytes: u64,
    /// Lookups served from the cache.
    pub hits: u64,
}

/// See the module docs.
pub struct BlockCache {
    inner: Mutex<Inner>,
    capacity: usize,
    next_id: AtomicU32,
    produced: AtomicU64,
    produced_bytes: AtomicU64,
    hits: AtomicU64,
}

impl BlockCache {
    /// A cache holding at most `capacity` chunks.
    pub fn new(capacity: usize) -> BlockCache {
        BlockCache {
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                order: VecDeque::new(),
            }),
            capacity: capacity.max(1),
            next_id: AtomicU32::new(0),
            produced: AtomicU64::new(0),
            produced_bytes: AtomicU64::new(0),
            hits: AtomicU64::new(0),
        }
    }

    /// A fresh id for one cached storage.
    pub fn new_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// The chunk `(id, index)`, produced by `make` if absent.
    pub fn get(
        &self,
        id: u32,
        index: u64,
        make: impl FnOnce() -> Result<Vec<u8>>,
    ) -> Result<Arc<Vec<u8>>> {
        let slot = {
            let mut g = self
                .inner
                .lock()
                .map_err(|_| ExtractError::format("block cache poisoned"))?;
            if let Some(s) = g.map.get(&(id, index)) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                s.clone()
            } else {
                let s: Slot = Arc::new(OnceLock::new());
                g.map.insert((id, index), s.clone());
                g.order.push_back((id, index));
                while g.order.len() > self.capacity {
                    if let Some(k) = g.order.pop_front() {
                        g.map.remove(&k);
                    }
                }
                s
            }
        };
        let r = slot.get_or_init(|| match make() {
            Ok(v) => {
                self.produced.fetch_add(1, Ordering::Relaxed);
                self.produced_bytes
                    .fetch_add(v.len() as u64, Ordering::Relaxed);
                Ok(Arc::new(v))
            }
            Err(e) => Err(e.to_string()),
        });
        match r {
            Ok(v) => Ok(v.clone()),
            Err(e) => Err(ExtractError::format(e.clone())),
        }
    }

    /// Counters so far.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            produced: self.produced.load(Ordering::Relaxed),
            produced_bytes: self.produced_bytes.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produced_once_and_evicted() {
        let c = BlockCache::new(2);
        let id = c.new_id();
        for _ in 0..3 {
            let v = c.get(id, 7, || Ok(vec![1, 2, 3]));
            assert_eq!(v.map(|v| v.len()).ok(), Some(3));
        }
        assert_eq!(c.stats().produced, 1);
        let _ = c.get(id, 8, || Ok(vec![]));
        let _ = c.get(id, 9, || Ok(vec![]));
        let _ = c.get(id, 7, || Ok(vec![9]));
        assert_eq!(c.stats().produced, 4);
    }

    #[test]
    fn concurrent_single_production() {
        let c = Arc::new(BlockCache::new(16));
        let id = c.new_id();
        let hs: Vec<_> = (0..8)
            .map(|_| {
                let c = c.clone();
                std::thread::spawn(move || {
                    c.get(id, 1, || {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        Ok(vec![0; 4])
                    })
                    .is_ok()
                })
            })
            .collect();
        for h in hs {
            assert!(h.join().unwrap_or(false));
        }
        assert_eq!(c.stats().produced, 1);
    }
}
