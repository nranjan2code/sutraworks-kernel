//! Filesystem Block Cache
//!
//! Implements a Write-Back Block Cache with LRU eviction.

use alloc::vec::Vec;
use alloc::sync::Arc;
use crate::arch::SpinLock;
use crate::fs::vfs::BlockDevice;

/// Cache Entry
#[derive(Clone)]
struct CacheEntry {
    sector: u32,
    data: [u8; 512],
    dirty: bool,
    last_access: u64,
}

/// Block Cache
pub struct BlockCache {
    device: Arc<dyn BlockDevice>,
    entries: Vec<CacheEntry>,
    capacity: usize,
}

impl BlockCache {
    /// Create a new Block Cache
    pub fn new(device: Arc<dyn BlockDevice>, capacity: usize) -> Self {
        Self {
            device,
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Synchronize all dirty blocks to the device
    pub fn sync(&mut self) -> Result<(), &'static str> {
        for entry in &mut self.entries {
            if entry.dirty {
                self.device.write_sector(entry.sector, &entry.data)?;
                entry.dirty = false;
            }
        }
        Ok(())
    }

    /// Get index of entry with sector, or None
    fn find_entry(&self, sector: u32) -> Option<usize> {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.sector == sector {
                return Some(i);
            }
        }
        None
    }

    /// Evict the least recently used entry
    fn evict(&mut self) -> Result<(), &'static str> {
        if self.entries.is_empty() { return Ok(()); }

        // Find LRU
        let mut lru_idx = 0;
        let mut min_access = u64::MAX;

        for (i, entry) in self.entries.iter().enumerate() {
            if entry.last_access < min_access {
                min_access = entry.last_access;
                lru_idx = i;
            }
        }

        // Write back if dirty
        if self.entries[lru_idx].dirty {
            let sector = self.entries[lru_idx].sector;
            let data = self.entries[lru_idx].data;
            self.device.write_sector(sector, &data)?;
        }

        // Remove
        self.entries.remove(lru_idx);
        Ok(())
    }
}

/// Thread-safe Block Cache Wrapper
pub struct CachedDevice {
    inner: SpinLock<BlockCache>,
}

impl CachedDevice {
    pub fn new(device: Arc<dyn BlockDevice>, capacity: usize) -> Self {
        Self {
            inner: SpinLock::new(BlockCache::new(device, capacity)),
        }
    }
    
    pub fn sync(&self) -> Result<(), &'static str> {
        self.inner.lock().sync()
    }
}

impl BlockDevice for CachedDevice {
    fn read_sector(&self, sector: u32, buf: &mut [u8]) -> Result<(), &'static str> {
        let mut cache = self.inner.lock();
        let time = crate::drivers::timer::uptime_ms(); // or generic counter

        if let Some(idx) = cache.find_entry(sector) {
            // Hit
            cache.entries[idx].last_access = time;
            buf.copy_from_slice(&cache.entries[idx].data);
            return Ok(());
        }

        // Miss
        if cache.entries.len() >= cache.capacity {
            cache.evict()?;
        }

        // Read from device
        let mut data = [0u8; 512];
        cache.device.read_sector(sector, &mut data)?;

        // Store
        cache.entries.push(CacheEntry {
            sector,
            data,
            dirty: false,
            last_access: time,
        });

        buf.copy_from_slice(&data);
        Ok(())
    }

    fn write_sector(&self, sector: u32, buf: &[u8]) -> Result<(), &'static str> {
        let mut cache = self.inner.lock();
        let time = crate::drivers::timer::uptime_ms();

        if let Some(idx) = cache.find_entry(sector) {
            // Hit - Update
            cache.entries[idx].data.copy_from_slice(buf);
            cache.entries[idx].dirty = true;
            cache.entries[idx].last_access = time;
            return Ok(());
        }

        // Miss
        if cache.entries.len() >= cache.capacity {
            cache.evict()?;
        }

        // Create new entry (Write-Allocate)
        let mut data = [0u8; 512];
        data.copy_from_slice(buf);
        
        cache.entries.push(CacheEntry {
            sector,
            data,
            dirty: true,
            last_access: time,
        });
        
        Ok(())
    }

    fn sync(&self) -> Result<(), &'static str> {
        self.inner.lock().sync()
    }
}
