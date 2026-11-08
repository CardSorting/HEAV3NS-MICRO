// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};

pub const DEFAULT_ARENA_CAPACITY_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
pub type ArenaWordOffset = usize;
pub type ArenaByteRange = (usize, usize);

/// Abstract contract for contiguous slab arena allocators.
pub trait IArenaSlab {
    fn capacity_bytes(&self) -> usize;
    fn allocated_bytes(&self) -> usize;
    fn snapshot(&self) -> SlabBufferSnapshot;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlabBufferSnapshot {
    #[serde(rename = "capacityBytes")]
    pub capacity_bytes: usize,
    #[serde(rename = "offsetWords")]
    pub offset_words: usize,
    #[serde(rename = "allocatedBytes")]
    pub allocated_bytes: usize,
    #[serde(rename = "resetCount")]
    pub reset_count: u64,
}

/// Zero-GC Contiguous Slab Allocator for mechanical sympathy and O(1) state rewind.
#[derive(Debug, Clone)]
pub struct ArenaAllocator {
    buffer: Vec<u8>,
    offset_words: usize,
    reset_count: u64,
}

impl Default for ArenaAllocator {
    fn default() -> Self {
        Self::new(DEFAULT_ARENA_CAPACITY_BYTES)
    }
}

impl ArenaAllocator {
    pub fn new(capacity_bytes: usize) -> Self {
        let aligned_cap = (capacity_bytes.max(64) + 3) & !3;
        Self {
            buffer: vec![0u8; aligned_cap],
            offset_words: 0,
            reset_count: 0,
        }
    }

    #[inline]
    pub fn capacity_words(&self) -> usize {
        self.buffer.len() / 4
    }

    #[inline]
    pub fn capacity_bytes(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub fn allocated_bytes(&self) -> usize {
        self.offset_words * 4
    }

    #[inline]
    pub const fn offset_words(&self) -> usize {
        self.offset_words
    }

    #[inline]
    pub const fn reset_count(&self) -> u64 {
        self.reset_count
    }

    pub fn allocate_node(&mut self, id: u32, flags: u32) -> Result<usize, String> {
        let ptr = self.offset_words;
        if ptr + 2 > self.capacity_words() {
            return Err(format!(
                "ArenaAllocator slab exhausted (capacity: {} bytes)",
                self.capacity_bytes()
            ));
        }
        let byte_offset = ptr * 4;
        self.buffer[byte_offset..byte_offset + 4].copy_from_slice(&id.to_le_bytes());
        self.buffer[byte_offset + 4..byte_offset + 8].copy_from_slice(&flags.to_le_bytes());
        self.offset_words += 2;
        Ok(ptr)
    }

    pub fn allocate_raw_bytes(&mut self, byte_len: usize) -> Result<usize, String> {
        let byte_offset = self.offset_words * 4;
        let words_needed = (byte_len + 3) / 4;
        if self.offset_words + words_needed > self.capacity_words() {
            return Err("ArenaAllocator slab exhausted for raw bytes allocation".to_string());
        }
        self.offset_words += words_needed;
        Ok(byte_offset)
    }

    pub fn allocate_str(&mut self, s: &str) -> Result<(usize, usize), String> {
        let bytes = s.as_bytes();
        let offset = self.allocate_raw_bytes(bytes.len())?;
        self.buffer[offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok((offset, bytes.len()))
    }

    pub fn read_str(&self, byte_offset: usize, byte_len: usize) -> Result<&str, String> {
        if byte_offset + byte_len > self.buffer.len() {
            return Err(format!("Out of bounds read_str: offset {byte_offset}, len {byte_len}"));
        }
        std::str::from_utf8(&self.buffer[byte_offset..byte_offset + byte_len])
            .map_err(|e| format!("Invalid UTF-8 in arena slab: {e}"))
    }

    pub fn set_offset(&mut self, offset_words: usize) -> Result<(), String> {
        if offset_words > self.capacity_words() {
            return Err(format!("Invalid offset_words {offset_words}"));
        }
        self.offset_words = offset_words;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.offset_words = 0;
        self.reset_count += 1;
    }

    pub fn snapshot(&self) -> SlabBufferSnapshot {
        SlabBufferSnapshot {
            capacity_bytes: self.capacity_bytes(),
            offset_words: self.offset_words,
            allocated_bytes: self.allocated_bytes(),
            reset_count: self.reset_count,
        }
    }
}
