use super::addr::*;
use super::page::*;

/// `FrameAllocator` clients may attempt to reserve a specific frame of memory.
/// This can fail for one of the reasons listed below.
pub enum FrameReserveError {
    /// The requested frame was either returned by an `allocate` call or
    /// previously reserved
    FrameInUse,
}

/// A physical frame allocator
///
/// # Safety
///
/// This trait is marked `unsafe` since pretty much the entire kernel relies on
/// its correctness for safety. Implementations must uphold the following
/// invariants:
///
///   - allocations return valid memory
///   - allocations do not return allocated or reserved frames
///   - `reserve` will not succeed on an allocated or reserved frame
pub unsafe trait FrameAllocator {
    /// Allocate one frame of physical address space, if available.
    fn allocate(&mut self) -> Option<Frame>;

    /// Return one allocated frame of physical address space.
    ///
    /// # Safety
    ///
    /// The frame must have been allocated and not deallocated since.
    unsafe fn deallocate(&mut self, frame: Frame);

    /// Reserve a specific frame, if possible.
    fn reserve(&mut self, frame: Frame) -> Result<(), FrameReserveError>;

    /// Return a frame reserved by `reserve`.
    ///
    /// # Safety
    ///
    /// The frame must have been successfully reserved by `reserve` and not
    /// returned by `unreserve` since.
    unsafe fn unreserve(&mut self, frame: Frame);
}

/// A very rudimentary allocator. Simply stores 1 bit per frame representing
/// whether it's available. Allocations search this bitmap for a free frame.
pub struct BitmapFrameAllocator<'a> {
    bitmap: &'a mut [u8],
    start_offset: usize,
}

impl<'a> BitmapFrameAllocator<'a> {
    /// Creates an allocator that allocates from `bitmap`. The first bit of
    /// `bitmap` represents the frame starting at address 0. Each subsequent bit
    /// represents the next frame. 1 means it's free, and 0 means it's used.
    ///
    /// # Safety
    ///
    /// `bitmap` must accurately reflect the state of memory at construction.
    /// All frames that must be preserved or which refer to invalid memory must
    /// be marked used. All frames marked free must be available for use and not used
    /// by other code.
    pub unsafe fn new(bitmap: &'a mut [u8]) -> BitmapFrameAllocator {
        BitmapFrameAllocator {
            bitmap,
            start_offset: 0,
        }
    }

    // Finds the first byte of `bitmap` after `offset` with an available slot.
    fn search_from_offset(&self, offset: usize) -> Option<usize> {
        for i in offset..self.bitmap.len() {
            if self.bitmap[i] > 0 {
                return Some(i);
            }
        }

        None
    }

    fn offsets_to_frame(byte_offset: usize, bit_offset: u32) -> Frame {
        Frame::new(PhysAddress::from_raw(
            (byte_offset as u64) * PAGE_SIZE.as_raw() * 8
                + (bit_offset as u64) * PAGE_SIZE.as_raw(),
        ))
    }

    fn frame_to_offsets(frame: Frame) -> (usize, u32) {
        let addr_raw = frame.start().as_raw();
        (
            (addr_raw / PAGE_SIZE.as_raw() / 8) as usize,
            ((addr_raw / PAGE_SIZE.as_raw()) % 8) as u32,
        )
    }

    fn deallocate_impl(&mut self, frame: Frame) {
        let (byte_offset, bit_offset) = Self::frame_to_offsets(frame);
        let mask = 1 << bit_offset;
        assert_eq!(self.bitmap[byte_offset] & mask, 0);
        self.bitmap[byte_offset] |= mask;
    }

    fn unreserve_impl(&mut self, frame: Frame) {
        let (byte_offset, bit_offset) = Self::frame_to_offsets(frame);
        let mask = 1 << bit_offset;
        assert_eq!(self.bitmap[byte_offset] & mask, 0);
        self.bitmap[byte_offset] |= mask;
    }
}

unsafe impl FrameAllocator for BitmapFrameAllocator<'_> {
    fn allocate(&mut self) -> Option<Frame> {
        let byte_offset = self
            .search_from_offset(self.start_offset)
            .or_else(|| self.search_from_offset(0))?;
        assert_ne!(self.bitmap[byte_offset], 0);
        let bit_offset = self.bitmap[byte_offset].trailing_zeros();
        self.bitmap[byte_offset] &= !(1 << bit_offset);

        Some(Self::offsets_to_frame(byte_offset, bit_offset))
    }

    unsafe fn deallocate(&mut self, frame: Frame) {
        self.deallocate_impl(frame)
    }

    fn reserve(&mut self, frame: Frame) -> Result<(), FrameReserveError> {
        let (byte_offset, bit_offset) = Self::frame_to_offsets(frame);
        let mask = 1 << bit_offset;

        let frame_is_available = self.bitmap[byte_offset] & mask > 0;
        if !frame_is_available {
            return Err(FrameReserveError::FrameInUse);
        }

        self.bitmap[byte_offset] &= !mask;
        Ok(())
    }

    unsafe fn unreserve(&mut self, frame: Frame) {
        self.unreserve_impl(frame)
    }
}

/// Initializes `bitmap` from `memory_map` in the format that
/// [`BitmapFrameAllocator`](self::BitmapFrameAllocator) expects. `bitmap` must
/// be large enough. Specifically, if the last entry in `memory_map` ends just
/// before address x, `bitmap` must have length at least x / 32768 (which is the
/// frame size, 4096, times the number of bits in a u8, 8).
pub fn fill_bitmap_from_map(bitmap: &mut [u8], memory_map: &crate::memory::Map) {
    use crate::memory::MemoryType;

    // The number of memory frames per byte of `bitmap`
    const FRAMES_PER_ENTRY: u64 = 8;
    // The number of memory bytes per byte of `bitmap`.
    const BYTES_PER_ENTRY: u64 = PAGE_SIZE.as_raw() * FRAMES_PER_ENTRY;

    assert!(
        bitmap.len() as u64
            >= ceil_divide(
                memory_map
                    .entries()
                    .last()
                    .unwrap()
                    .extent
                    .end_address()
                    .as_raw(),
                BYTES_PER_ENTRY
            )
    );

    for x in bitmap.iter_mut() {
        *x = 0;
    }

    for e in memory_map.entries() {
        if e.mem_type != MemoryType::Available {
            continue;
        }

        // Only mark the inner frame-aligned part as available.
        let maybe_avail_extent = e.extent.shrink_to_alignment(PAGE_SIZE.as_raw());
        if maybe_avail_extent.is_none() {
            continue;
        }

        let avail_extent = maybe_avail_extent.unwrap();

        // Ensure `bitmap` is large enough.
        assert!(bitmap.len() as u64 >= avail_extent.end_address().as_raw() / BYTES_PER_ENTRY);

        // Get the inner part that is aligned to byte boundaries in `bitmap`. We
        // can fill this section of the bitmap more efficiently.
        let maybe_aligned_extent = e.extent.shrink_to_alignment(BYTES_PER_ENTRY);

        // For simplicity, skip entries that are too small to fill an entire
        // byte of `bitmap`. TODO: remove this restriction.
        if maybe_aligned_extent.is_none() {
            continue;
        }

        let aligned_extent = maybe_aligned_extent.unwrap();

        if let Some(aligned_extent) = maybe_aligned_extent {
            for x in (aligned_extent.address().as_raw()..aligned_extent.end_address().as_raw())
                .step_by(BYTES_PER_ENTRY as usize)
            {
                let byte_offset = x / BYTES_PER_ENTRY;

                // u8::MAX has all bits set.
                bitmap[byte_offset as usize] = u8::MAX;
            }
        }

        // Now fill `bitmap` for the leading and trailing ends.

        if let Some(left_end) = avail_extent.left_difference(aligned_extent) {
            // We should only have to touch one bitmap byte, and only the last n
            // bits of it at that.
            assert!(left_end.end_address().is_aligned_to(BYTES_PER_ENTRY));
            assert!(left_end.length().as_raw() / PAGE_SIZE.as_raw() < FRAMES_PER_ENTRY);

            let byte_offset = left_end.address().as_raw() / BYTES_PER_ENTRY;
            let set_bits = left_end.length().as_raw() / PAGE_SIZE.as_raw();

            bitmap[byte_offset as usize] |= set_most_significant_bits(set_bits as u8);
        }

        if let Some(right_end) = avail_extent.right_difference(aligned_extent) {
            // Like the above, but the first n bits.
            assert!(right_end.address().is_aligned_to(BYTES_PER_ENTRY));
            assert!(right_end.length().as_raw() / PAGE_SIZE.as_raw() < FRAMES_PER_ENTRY);

            let byte_offset = right_end.address().as_raw() / BYTES_PER_ENTRY;
            let set_bits = right_end.length().as_raw() / PAGE_SIZE.as_raw();

            bitmap[byte_offset as usize] |= set_least_significant_bits(set_bits as u8);
        }
    }
}

fn set_most_significant_bits(num_bits: u8) -> u8 {
    if num_bits == 0 {
        0
    } else if num_bits < 8 {
        u8::MAX << (8 - num_bits)
    } else {
        u8::MAX
    }
}

fn set_least_significant_bits(num_bits: u8) -> u8 {
    if num_bits == 0 {
        0
    } else if num_bits < 8 {
        u8::MAX >> (8 - num_bits)
    } else {
        u8::MAX
    }
}

fn ceil_divide(x: u64, divisor: u64) -> u64 {
    (x + divisor - 1) / divisor
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::memory;

    use std::vec::Vec;

    #[test]
    fn most_significant_bits() {
        assert_eq!(set_most_significant_bits(0), 0b00000000);
        assert_eq!(set_most_significant_bits(1), 0b10000000);
        assert_eq!(set_most_significant_bits(2), 0b11000000);
        assert_eq!(set_most_significant_bits(3), 0b11100000);
        assert_eq!(set_most_significant_bits(4), 0b11110000);
        assert_eq!(set_most_significant_bits(5), 0b11111000);
        assert_eq!(set_most_significant_bits(6), 0b11111100);
        assert_eq!(set_most_significant_bits(7), 0b11111110);
        assert_eq!(set_most_significant_bits(8), 0b11111111);
    }

    #[test]
    fn least_significant_bits() {
        assert_eq!(set_least_significant_bits(0), 0b00000000);
        assert_eq!(set_least_significant_bits(1), 0b00000001);
        assert_eq!(set_least_significant_bits(2), 0b00000011);
        assert_eq!(set_least_significant_bits(3), 0b00000111);
        assert_eq!(set_least_significant_bits(4), 0b00001111);
        assert_eq!(set_least_significant_bits(5), 0b00011111);
        assert_eq!(set_least_significant_bits(6), 0b00111111);
        assert_eq!(set_least_significant_bits(7), 0b01111111);
        assert_eq!(set_least_significant_bits(8), 0b11111111);
    }

    #[test]
    fn fill_bitmap_single_element() {
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(0, PAGE_SIZE.as_raw() * 8)].iter().copied()
            )),
            &[0b11111111]
        );
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(0, PAGE_SIZE.as_raw() * 16)].iter().copied()
            )),
            &[0b11111111, 0b11111111]
        );
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(0, PAGE_SIZE.as_raw() * 24)].iter().copied()
            )),
            &[0b11111111, 0b11111111, 0b11111111]
        );

        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(PAGE_SIZE.as_raw(), PAGE_SIZE.as_raw() * 24)]
                    .iter()
                    .copied()
            )),
            &[0b11111110, 0b11111111, 0b11111111]
        );
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(0, PAGE_SIZE.as_raw() * 23)].iter().copied()
            )),
            &[0b11111111, 0b11111111, 0b01111111]
        );
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(PAGE_SIZE.as_raw(), PAGE_SIZE.as_raw() * 23)]
                    .iter()
                    .copied()
            )),
            &[0b11111110, 0b11111111, 0b01111111]
        );

        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(PAGE_SIZE.as_raw() * 7, PAGE_SIZE.as_raw() * 17)]
                    .iter()
                    .copied()
            )),
            &[0b10000000, 0b11111111, 0b00000001]
        );
    }

    #[test]
    fn fill_bitmap_multiple_elements() {
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [
                    (0, PAGE_SIZE.as_raw() * 8),
                    (PAGE_SIZE.as_raw() * 16, PAGE_SIZE.as_raw() * 24)
                ]
                .iter()
                .copied()
            )),
            &[0b11111111, 0b00000000, 0b11111111]
        );

        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [
                    (0, PAGE_SIZE.as_raw() * 9),
                    (PAGE_SIZE.as_raw() * 15, PAGE_SIZE.as_raw() * 24)
                ]
                .iter()
                .copied()
            )),
            &[0b11111111, 0b10000001, 0b11111111]
        );
    }

    #[test]
    fn fill_bitmap_off_page_boundary() {
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(0, PAGE_SIZE.as_raw() * 8 + 1),].iter().copied()
            )),
            &[0b11111111, 0b00000000]
        );

        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [
                    (0, PAGE_SIZE.as_raw() * 8 + 1),
                    (PAGE_SIZE.as_raw() * 16 - 1, PAGE_SIZE.as_raw() * 24),
                ]
                .iter()
                .copied()
            )),
            &[0b11111111, 0b00000000, 0b11111111]
        );
    }

    #[test]
    fn fill_bitmap_filters_unavailable() {
        assert_eq!(
            fill_bitmap(&memory::Map::from_entries(
                [
                    memory::MapEntry {
                        extent: memory::PhysExtent::from_raw_range_exclusive(
                            0,
                            PAGE_SIZE.as_raw() * 8
                        ),
                        mem_type: memory::MemoryType::Acpi
                    },
                    memory::MapEntry {
                        extent: memory::PhysExtent::from_raw_range_exclusive(
                            PAGE_SIZE.as_raw() * 8,
                            PAGE_SIZE.as_raw() * 16
                        ),
                        mem_type: memory::MemoryType::Available
                    }
                ]
                .iter()
                .copied()
            )),
            &[0b00000000, 0b11111111]
        );
    }

    fn map_from_pairs<T: IntoIterator<Item = (u64, u64)>>(pairs: T) -> memory::Map {
        map_from_extents(
            pairs
                .into_iter()
                .map(|(begin, end)| memory::PhysExtent::from_raw_range_exclusive(begin, end)),
        )
    }

    fn map_from_extents<T: IntoIterator<Item = memory::PhysExtent>>(extents: T) -> memory::Map {
        memory::Map::from_entries(extents.into_iter().map(|extent| memory::MapEntry {
            extent,
            mem_type: memory::MemoryType::Available,
        }))
    }

    fn fill_bitmap(memory_map: &memory::Map) -> Vec<u8> {
        let total_memory = memory_map
            .entries()
            .last()
            .unwrap()
            .extent
            .end_address()
            .as_raw();

        let mut bitmap = Vec::new();
        bitmap.resize(
            ceil_divide(total_memory, PAGE_SIZE.as_raw() * 8) as usize,
            0,
        );

        fill_bitmap_from_map(&mut bitmap, memory_map);
        return bitmap;
    }

    #[test]
    fn bitmap_allocator_uses_all_available_memory() {
        let mut bitmap = [0b11111111, 0b11111111, 0b11111111];
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };

        let mut allocated_frames = std::collections::BTreeSet::new();

        for _i in 0..24 {
            let frame = allocator.allocate().unwrap();
            assert!(allocated_frames.insert(frame));
        }

        assert_eq!(allocator.allocate(), None);
    }
}
