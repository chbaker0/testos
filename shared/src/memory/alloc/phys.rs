use crate::memory::addr::*;
use crate::memory::page::*;

use core::convert::TryInto;

/// `FrameAllocator` clients may attempt to reserve a specific frame of memory.
/// This can fail for one of the reasons listed below.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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
    fn allocate(&mut self) -> Option<Frame> {
        self.allocate_range(0).map(|r| r.first())
    }

    /// Allocate 2^order frames aligned to 2^order, if available.
    fn allocate_range(&mut self, order: usize) -> Option<FrameRange>;

    /// Return one allocated frame of physical address space.
    ///
    /// # Safety
    ///
    /// `frame` must have been returned by allocate and not deallocated since.
    fn deallocate(&mut self, frame: Frame) {
        self.deallocate_range(FrameRange::one(frame))
    }

    /// Return several allocated frames of physical address space.
    ///
    /// # Safety
    ///
    /// `range` must have been returned by allocate_range and not deallocated
    /// since.
    fn deallocate_range(&mut self, range: FrameRange);

    /// Reserve a specific frame, if possible.
    fn reserve(&mut self, frame: Frame) -> Result<(), FrameReserveError>;

    /// Return a frame reserved by `reserve`.
    ///
    /// # Safety
    ///
    /// The frame must have been successfully reserved by `reserve` and not
    /// returned by `unreserve` since.
    fn unreserve(&mut self, frame: Frame);
}

/// Allocates successive frames from a given set. This can be "unwrapped" to get
/// back the unallocated frames.
///
/// Useful for allocating initial memory before using a better allocator, or
/// various other allocation patterns. Importantly, it's not possible to return
/// frames allocated by this. It's best for allocations that will last until
/// shutdown.
///
/// It does not implement `FrameAllocator` because of these restrictions.
#[derive(Debug)]
pub struct BumpFrameAllocator {
    remain: Option<FrameRange>,
}

impl BumpFrameAllocator {
    pub fn new(frames: FrameRange) -> Self {
        BumpFrameAllocator {
            remain: Some(frames),
        }
    }

    pub fn allocate(&mut self) -> Option<Frame> {
        let remain = self.remain?;
        let frame = remain.first();
        self.remain = FrameRange::new(frame.next(1)?, remain.count() - 1);
        Some(frame)
    }

    /// Get the remaining frames.
    pub fn unwrap(self) -> Option<FrameRange> {
        self.remain
    }
}

/// A very rudimentary allocator. Simply stores 1 bit per frame representing
/// whether it's available. Allocations search this bitmap for a free frame.
#[derive(Debug)]
pub struct BitmapFrameAllocator<'a> {
    bitmap: &'a mut [u8],
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
        BitmapFrameAllocator { bitmap }
    }

    /// Add a new frame that wasn't present in the initial bitmap. Intended for
    /// yielding frames used during bootstrapping so they can be used later.
    ///
    /// # Safety
    ///
    /// `frame` must obviously be a valid frame of physical memory. In addition,
    /// it must not have been known by the allocator when constructed.
    pub unsafe fn add_new_frame(&mut self, frame: Frame) {
        self.unreserve_impl(frame)
    }

    // Finds the first byte of `bitmap` after `offset` with an available slot.
    #[allow(dead_code)]
    fn search_from_offset(&self, offset: usize) -> Option<usize> {
        (offset..self.bitmap.len()).find(|&i| self.bitmap[i] > 0)
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
    fn allocate_range(&mut self, order: usize) -> Option<FrameRange> {
        // An order of 24 gives a size of 8 MiB. Let this be the max size.
        assert!(order <= 24);
        let size = 1 << order;

        // Must find `size` contiguous free frames, aligned to `size`. For
        // `size` = 1, this corresponds to finding any 1 bit in the bitmap. For
        // `size` <= 8, a correctly aligned range will be contained within one
        // bitmap byte. If `size` >= 8, a range will be several bytes of
        // `u8::MAX`.
        //
        // Handle `size` < 8 first. We can handle `size` >= 8 on the byte level
        // instead.

        if size < 8 {
            for i in 0..self.bitmap.len() {
                let byte = &mut self.bitmap[i];
                if *byte == 0 {
                    continue;
                }

                if let Some(boff) = find_bit_group(*byte, size) {
                    let mask: u8 = ((1 << size) - 1).try_into().unwrap();
                    *byte &= !(mask << boff);
                    return FrameRange::new(Self::offsets_to_frame(i, boff.into()), size as u64);
                }
            }

            return None;
        }

        assert!(size >= 8);
        let byte_len = size / 8;

        // For sizes >= 8, an allocation will correspond to a power-of-two
        // length of bytes in the bitmap, aligned appropriately.

        'outer: for i in (0..self.bitmap.len()).step_by(byte_len) {
            if i + byte_len > self.bitmap.len() {
                return None;
            }

            for j in i..i + byte_len {
                if self.bitmap[j] != u8::MAX {
                    // Not every frame is available in this range. Try the next
                    // one.
                    continue 'outer;
                }
            }

            // Every frame in this range is available. Allocate it.
            for j in i..i + byte_len {
                self.bitmap[j] = 0;
            }

            return FrameRange::new(Self::offsets_to_frame(i, 0), size as u64);
        }

        unreachable!();
    }

    fn deallocate(&mut self, frame: Frame) {
        self.deallocate_impl(frame)
    }

    fn deallocate_range(&mut self, range: FrameRange) {
        for frame in range.iter() {
            self.deallocate(frame);
        }
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

    fn unreserve(&mut self, frame: Frame) {
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

    for avail_frames in crate::memory::iter_map_frames(memory_map.iter_type(MemoryType::Available))
    {
        // Ensure `bitmap` is large enough.
        assert!(bitmap.len() as u64 >= avail_frames.count() / FRAMES_PER_ENTRY);

        // For each FrameRange, we need to do at least one of the following, in
        // order from lowest to highest byte in the bitmap:
        // * set some bits at the end of a byte,
        // * set all bits for some range of bytes,
        // * set some bits at the beginning of a byte.
        //
        // Obviously, all bytes we touch will be contiguous for one FrameRange.

        let first = avail_frames.first().index();
        let end = avail_frames.last().index() + 1;

        let first_aligned = first.next_multiple_of(FRAMES_PER_ENTRY);
        let end_aligned = end / FRAMES_PER_ENTRY * FRAMES_PER_ENTRY;

        for i in (first_aligned..end_aligned).step_by(FRAMES_PER_ENTRY as usize) {
            let byte_offset = i / FRAMES_PER_ENTRY;
            bitmap[byte_offset as usize] = u8::MAX;
        }

        // Now fill `bitmap` for the leading and trailing ends.

        if first != first_aligned {
            let first_byte = (first / FRAMES_PER_ENTRY) as usize;
            assert_eq!(first_byte, (first_aligned / FRAMES_PER_ENTRY - 1) as usize);
            bitmap[first_byte] |=
                set_most_significant_bits((first_aligned - first).try_into().unwrap());
        }

        if end != end_aligned {
            let last_byte = (end / FRAMES_PER_ENTRY) as usize;
            assert_eq!(
                last_byte,
                ((end_aligned - 1) / FRAMES_PER_ENTRY + 1) as usize
            );
            bitmap[last_byte] |=
                set_least_significant_bits((end - end_aligned).try_into().unwrap());
        }
    }
}

/// Finds `len` set bits in `byte`, aligned to `len`. Returns the bit offset
/// from the least significant bit.
///
/// Example: `len` is 2, will match the following bytes (where x any bit):
/// - 0bxxxxxx11 -> Some(0)
/// - 0bxxxx1100 -> Some(2)
/// - 0bxx110000 -> Some(4)
/// - 0b11000000 -> Some(6)
///
/// # Panics
///
/// Panics if `len` >= 8 or if `len` is not a power of two.
fn find_bit_group(byte: u8, len: usize) -> Option<u8> {
    assert!(len < 8);
    assert!(len.is_power_of_two());

    let mask = ((len << 1) - 1) as u8;
    let mut shift = 0;

    while shift < 8 {
        if (byte & (mask << shift)) >> shift == mask {
            return Some(shift);
        }
        shift += len as u8;
    }

    None
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
    fn find_bit_groups() {
        assert_eq!(find_bit_group(0b00000001, 1), Some(0));
        assert_eq!(find_bit_group(0b00000011, 2), Some(0));
        assert_eq!(find_bit_group(0b00001111, 4), Some(0));

        assert_eq!(find_bit_group(0b10000000, 1), Some(7));
        assert_eq!(find_bit_group(0b11000000, 2), Some(6));
        assert_eq!(find_bit_group(0b11110000, 4), Some(4));

        assert_eq!(find_bit_group(0b00110000, 2), Some(4));
        assert_eq!(find_bit_group(0b00001100, 2), Some(2));

        assert_eq!(find_bit_group(0b11111111, 2), Some(0));
        assert_eq!(find_bit_group(0b11111100, 2), Some(2));
        assert_eq!(find_bit_group(0b11110000, 2), Some(4));

        assert_eq!(find_bit_group(0b01010101, 2), None);
        assert_eq!(find_bit_group(0b11101110, 4), None);
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
    fn bitmap_allocator_returns_correct_available_frames() {
        // In each byte, the LSB represents the first frame in the range of 8
        // frames, and the MSB represents the last.
        let mut bitmap = [0b00100000, 0b00010000, 0b00000010];
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };
        let mut allocated_frames = std::collections::BTreeSet::new();

        assert!(allocated_frames.insert(allocator.allocate().unwrap()));
        assert!(allocated_frames.insert(allocator.allocate().unwrap()));
        assert!(allocated_frames.insert(allocator.allocate().unwrap()));

        assert_eq!(
            allocated_frames,
            vec![
                Frame::new(PhysAddress::from_zero(PAGE_SIZE * 5u64)),
                Frame::new(PhysAddress::from_zero(PAGE_SIZE * 12u64)),
                Frame::new(PhysAddress::from_zero(PAGE_SIZE * 17u64))
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn bitmap_allocator_does_not_return_reserved_frame() {
        let mut bitmap = [0b01000010];
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };

        allocator
            .reserve(Frame::new(PhysAddress::from_zero(PAGE_SIZE * 1u64)))
            .unwrap();
        assert_eq!(
            allocator.allocate().unwrap(),
            Frame::new(PhysAddress::from_zero(PAGE_SIZE * 6u64))
        );
        assert_eq!(allocator.allocate(), None);

        allocator.unreserve(Frame::new(PhysAddress::from_zero(PAGE_SIZE * 1u64)));
        assert_eq!(
            allocator.allocate().unwrap(),
            Frame::new(PhysAddress::from_zero(PAGE_SIZE * 1u64))
        );
        assert_eq!(allocator.allocate(), None);
    }

    #[test]
    fn bitmap_allocator_returns_freed_frame() {
        let mut bitmap = [0b01000010];
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };

        let frame1 = allocator.allocate().unwrap();
        let frame2 = allocator.allocate().unwrap();
        assert_eq!(allocator.allocate(), None);

        allocator.deallocate(frame2);
        assert_eq!(allocator.allocate().unwrap(), frame2);

        allocator.deallocate(frame1);
        assert_eq!(allocator.allocate().unwrap(), frame1);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn bitmap_allocator_uses_all_available_memory(mut bitmap in any::<Vec<u8>>()) {
            let free_frame_count = bitmap
                .iter()
                .copied()
                .map(u8::count_ones)
                .fold(0, |acc, x| acc + x as u64);

            let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };
            let mut allocated_frames = std::collections::BTreeSet::new();

            // Check that all available frames could be allocated and are unique.
            for _i in 0..free_frame_count {
                let frame = allocator.allocate().unwrap();
                prop_assert!(allocated_frames.insert(frame));
            }

            // Check that the allocator fails when all memory is used.
            prop_assert_eq!(allocator.allocate(), None);
        }
    }
}
