use crate::memory::addr::*;
use crate::memory::page::*;

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
    unsafe fn deallocate(&mut self, frame: Frame) {
        // SAFETY: forwarded from this fn's contract — `frame` was returned by
        // `allocate` and not deallocated since, so the one-frame range built
        // from it satisfies `deallocate_range`'s contract.
        unsafe { self.deallocate_range(FrameRange::one(frame)) }
    }

    /// Return several allocated frames of physical address space.
    ///
    /// # Safety
    ///
    /// `range` must have been returned by allocate_range and not deallocated
    /// since.
    unsafe fn deallocate_range(&mut self, range: FrameRange);

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
    pub unsafe fn new(bitmap: &'a mut [u8]) -> BitmapFrameAllocator<'a> {
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

// SAFETY: `allocate_range` only ever clears bits it finds set (never hands
// out a frame whose bit was already 0, i.e. already allocated/reserved), and
// `reserve` fails (rather than clearing) when the target bit is already 0.
// Both `deallocate`/`unreserve` (via `deallocate_impl`/`unreserve_impl`)
// assert the bit is currently 0 before setting it, so double-free/double-
// unreserve panics rather than silently marking an in-use frame free twice.
// Together these satisfy `FrameAllocator`'s documented invariants.
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

    unsafe fn deallocate(&mut self, frame: Frame) {
        self.deallocate_impl(frame)
    }

    unsafe fn deallocate_range(&mut self, range: FrameRange) {
        for frame in range.iter() {
            // SAFETY: forwarded from this fn's contract — every frame in
            // `range` was returned by `allocate_range` and not deallocated
            // since, so each one satisfies `deallocate`'s contract.
            unsafe { self.deallocate(frame) };
        }
    }

    fn reserve(&mut self, frame: Frame) -> Result<(), FrameReserveError> {
        let (byte_offset, bit_offset) = Self::frame_to_offsets(frame);
        let mask = 1 << bit_offset;

        let len = self.bitmap.len();
        let bitmap_byte = self
            .bitmap
            .get_mut(byte_offset)
            .unwrap_or_else(|| panic!("frame {frame:?} exceeded bitmap size {len}"));
        let frame_is_available = *bitmap_byte & mask > 0;
        if !frame_is_available {
            return Err(FrameReserveError::FrameInUse);
        }

        *bitmap_byte &= !mask;
        Ok(())
    }

    unsafe fn unreserve(&mut self, frame: Frame) {
        self.unreserve_impl(frame)
    }
}

/// Initializes `bitmap` from `memory_map` in the format that
/// [`BitmapFrameAllocator`](self::BitmapFrameAllocator) expects. `bitmap` must
/// be large enough. Specifically, if the highest `Available` entry in
/// `memory_map` ends just before address x, `bitmap` must have length at least
/// x / 32768 (the frame size, 4096, times the number of bits in a u8, 8). Only
/// `Available` frames are recorded, so non-RAM regions above the last usable
/// RAM (e.g. high MMIO/reserved holes) need not be covered.
pub fn fill_bitmap_from_map(bitmap: &mut [u8], memory_map: &crate::memory::Map) {
    use crate::memory::MemoryType;

    // The number of memory frames per byte of `bitmap`
    const FRAMES_PER_ENTRY: u64 = 8;
    // The number of memory bytes per byte of `bitmap`.
    const BYTES_PER_ENTRY: u64 = PAGE_SIZE.as_raw() * FRAMES_PER_ENTRY;

    // Only `Available` frames are recorded below, so the bitmap only needs to
    // reach the highest usable-RAM address — not the end of the last map entry,
    // which on real UEFI maps is a high MMIO/reserved hole near the top of the
    // address space.
    let highest_available_end = memory_map
        .iter_type(MemoryType::Available)
        .map(|e| e.extent.end_address().as_raw())
        .max()
        .unwrap_or(0);
    assert!(bitmap.len() as u64 >= ceil_divide(highest_available_end, BYTES_PER_ENTRY));

    for x in bitmap.iter_mut() {
        *x = 0;
    }

    for avail_frames in crate::memory::iter_map_frames(memory_map.iter_type(MemoryType::Available))
    {
        mark_frames_free(bitmap, avail_frames);
    }
}

/// The number of memory frames tracked by one byte of a bitmap.
const FRAMES_PER_ENTRY: u64 = 8;

/// Marks exactly the frames in `frames` as free in `bitmap`, leaving every
/// other bit untouched.
///
/// Split out of [`fill_bitmap_from_map`] so it can be stated — and proved —
/// on its own terms: a `FrameRange` in, a set of bits out, no memory map
/// involved. See `verify::mark_frames_free_marks_exactly_its_frames`.
///
/// This walks the touched bytes uniformly rather than special-casing a
/// leading partial byte, a run of whole bytes, and a trailing partial byte.
/// The three-way split it replaces had two defects that the uniform form
/// cannot express (see `docs/kani-findings.md`, "fill_bitmap_from_map"): it
/// underflowed computing the trailing byte index when the range ended below
/// frame 8, and — the dangerous one — a range confined to a single byte was
/// widened to the *whole* byte, marking frames free that no `Available` region
/// covered. A whole byte still costs one store here, since `lo == 0 && hi == 8`
/// yields `u8::MAX`.
///
/// # Panics
///
/// Panics if `frames` reaches past the end of `bitmap`.
fn mark_frames_free(bitmap: &mut [u8], frames: FrameRange) {
    let first = frames.first().index();
    // `FrameRange` is non-empty and addressable by construction, so `end` is
    // at least `first + 1` and cannot overflow.
    let end = first + frames.count();

    let first_byte = (first / FRAMES_PER_ENTRY) as usize;
    let last_byte = ((end - 1) / FRAMES_PER_ENTRY) as usize;
    assert!(
        last_byte < bitmap.len(),
        "frame range {frames:?} reaches past the end of a {}-byte bitmap",
        bitmap.len()
    );

    for (i, bits) in bitmap[first_byte..=last_byte].iter_mut().enumerate() {
        // Frames this byte covers, as absolute indices.
        let byte_first_frame = (first_byte + i) as u64 * FRAMES_PER_ENTRY;
        let byte_end_frame = byte_first_frame + FRAMES_PER_ENTRY;

        // Clip the range to this byte, then rebase to bit positions 0..=8.
        // The clip is what keeps a range that starts or ends mid-byte from
        // spilling onto neighbouring frames.
        let lo = first.max(byte_first_frame) - byte_first_frame;
        let hi = end.min(byte_end_frame) - byte_first_frame;

        *bits |= set_bit_range(lo as u8, hi as u8);
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

/// A byte with bits `lo..hi` set and all others clear. `lo <= hi <= 8`; an
/// empty range yields `0`.
///
/// This is the one bit-shape [`mark_frames_free`] needs: a run of frames
/// clipped to a single bitmap byte. Expressing both partial ends and the
/// whole-byte case as one range is what lets that function drop its
/// leading/middle/trailing special cases.
fn set_bit_range(lo: u8, hi: u8) -> u8 {
    debug_assert!(lo <= hi && hi <= 8);
    set_least_significant_bits(hi) & !set_least_significant_bits(lo)
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
    x.div_ceil(divisor)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::memory;

    use std::vec::Vec;


    #[test]
    fn bit_ranges() {
        // Empty ranges.
        assert_eq!(set_bit_range(0, 0), 0b00000000);
        assert_eq!(set_bit_range(8, 8), 0b00000000);

        // Anchored low, anchored high, and the whole byte.
        assert_eq!(set_bit_range(0, 1), 0b00000001);
        assert_eq!(set_bit_range(0, 4), 0b00001111);
        assert_eq!(set_bit_range(7, 8), 0b10000000);
        assert_eq!(set_bit_range(4, 8), 0b11110000);
        assert_eq!(set_bit_range(0, 8), 0b11111111);

        // Interior ranges: the case the old leading/trailing split could not
        // express, and silently widened to the whole byte.
        assert_eq!(set_bit_range(1, 4), 0b00001110);
        assert_eq!(set_bit_range(2, 6), 0b00111100);
        assert_eq!(set_bit_range(3, 4), 0b00001000);
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

    /// Regression: a region ending below frame 8 used to underflow computing
    /// the trailing byte index. See `docs/kani-findings.md`.
    #[test]
    fn fill_bitmap_region_ending_in_first_byte() {
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(0, PAGE_SIZE.as_raw() * 4)].iter().copied()
            )),
            &[0b00001111]
        );
    }

    /// Regression: a region confined to a single bitmap byte, touching
    /// neither end of it, used to be widened to the whole byte — marking
    /// frames free that no `Available` region covered.
    #[test]
    fn fill_bitmap_region_inside_one_byte() {
        assert_eq!(
            fill_bitmap(&map_from_pairs(
                [(PAGE_SIZE.as_raw(), PAGE_SIZE.as_raw() * 4)].iter().copied()
            )),
            &[0b00001110]
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
        // SAFETY: `bitmap` is a local test array, not backing any real
        // memory; there's nothing else for its "free" bits to conflict with.
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
        // SAFETY: `bitmap` is a local test array, not backing any real
        // memory; there's nothing else for its "free" bits to conflict with.
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };

        allocator
            .reserve(Frame::new(PhysAddress::from_zero(PAGE_SIZE * 1u64)))
            .unwrap();
        assert_eq!(
            allocator.allocate().unwrap(),
            Frame::new(PhysAddress::from_zero(PAGE_SIZE * 6u64))
        );
        assert_eq!(allocator.allocate(), None);

        // SAFETY: this frame was reserved by the `reserve` call above and has
        // not been unreserved since.
        unsafe { allocator.unreserve(Frame::new(PhysAddress::from_zero(PAGE_SIZE * 1u64))) };
        assert_eq!(
            allocator.allocate().unwrap(),
            Frame::new(PhysAddress::from_zero(PAGE_SIZE * 1u64))
        );
        assert_eq!(allocator.allocate(), None);
    }

    #[test]
    fn bitmap_allocator_returns_freed_frame() {
        let mut bitmap = [0b01000010];
        // SAFETY: `bitmap` is a local test array, not backing any real
        // memory; there's nothing else for its "free" bits to conflict with.
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };

        let frame1 = allocator.allocate().unwrap();
        let frame2 = allocator.allocate().unwrap();
        assert_eq!(allocator.allocate(), None);

        // SAFETY: `frame2` was just returned by `allocate` and not yet
        // deallocated.
        unsafe { allocator.deallocate(frame2) };
        assert_eq!(allocator.allocate().unwrap(), frame2);

        // SAFETY: `frame1` was returned by `allocate` and not yet deallocated.
        unsafe { allocator.deallocate(frame1) };
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

            // SAFETY: `bitmap` is a proptest-generated local array, not
            // backing any real memory; there's nothing else for its "free"
            // bits to conflict with.
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

#[cfg(kani)]
mod verify {
    //! Kani proof harnesses for [`crate::memory::alloc::phys`].
    //!
    //! `FrameAllocator` is an `unsafe trait` whose documented invariants are
    //! blunt: *"allocations do not return allocated or reserved frames"* and
    //! *"`reserve` will not succeed on an allocated or reserved frame"*. Handing
    //! out a frame twice is not a wrong answer — it is a use-after-free of
    //! physical memory, aliasing two independent kernel data structures onto the
    //! same page. Every safety comment in `mm.rs` that says "frames not in use
    //! anywhere else" bottoms out here.
    //!
    //! That invariant is a *global* property of the bitmap, which is exactly the
    //! shape of claim a bounded model checker can settle and a randomized test
    //! cannot. The harnesses below take a fully symbolic bitmap — every bit of
    //! every byte unconstrained — and prove the invariant over all of it at once.

    use super::*;

    /// A symbolic bitmap. Two bytes is enough to cover every case the allocator
    /// distinguishes: sub-byte groups (`size < 8`), whole-byte groups
    /// (`size >= 8`), and the boundary between them — while keeping the search
    /// space small enough to settle quickly. `size >= 8` paths that need more
    /// bytes are covered by the `allocate_range_*_bytes_*` harnesses below.
    const BITMAP_BYTES: usize = 2;

    /// Is the frame at `index` marked free in `bitmap`? A `1` bit means free (see
    /// `BitmapFrameAllocator::new`'s doc comment). This is the specification of
    /// what the bitmap *means*; none of the code under proof is reused.
    fn bit_is_free(bitmap: &[u8], index: usize) -> bool {
        bitmap[index / 8] & (1u8 << (index % 8)) != 0
    }

    // ---------------------------------------------------------------------------
    // find_bit_group — the sub-byte free-run search
    //
    // This is the single function standing between `allocate_range` and handing
    // out an already-allocated frame: it claims to find `len` *set* bits, aligned
    // to `len`, and `allocate_range` then clears exactly `len` bits at the offset
    // it returns. If it can ever report an offset whose run is not entirely set,
    // `allocate_range` clears — i.e. allocates — a bit that was already zero.
    // ---------------------------------------------------------------------------

    /// `find_bit_group(byte, len)` must return an offset at which *all* `len` bits
    /// are set, aligned to `len` and within the byte.
    ///
    /// KNOWN FAILURE — see `docs/kani-findings.md` ("find_bit_group mask"). The
    /// mask is computed as `(len << 1) - 1`, which is correct for `len` 1 and 2
    /// but yields `0b111` for `len == 4` instead of `0b1111`: a three-bit mask
    /// used to check a four-bit run. The unit tests in `phys.rs` pass only because
    /// every `len == 4` case they try happens to have the fourth bit agree with
    /// the other three.
    #[kani::proof]
    #[kani::unwind(9)]
    fn find_bit_group_returns_a_fully_free_run() {
        let byte: u8 = kani::any();
        // `find_bit_group` asserts `len < 8` and `len.is_power_of_two()`, so 1, 2
        // and 4 are its entire legal domain.
        let len: usize = kani::any();
        kani::assume(len == 1 || len == 2 || len == 4);

        if let Some(offset) = find_bit_group(byte, len) {
            let offset = offset as usize;

            assert!(offset % len == 0, "the run is aligned to its own size");
            assert!(offset + len <= 8, "the run fits inside the byte");

            // The claim that matters: every bit in the reported run is free.
            // `allocate_range` will clear all `len` of them.
            for i in 0..len {
                assert!(
                    byte & (1u8 << (offset + i)) != 0,
                    "find_bit_group reported a run containing an already-used frame"
                );
            }
        }
    }

    /// The completeness direction: if an aligned, fully-free run exists,
    /// `find_bit_group` must find one. A false negative is not a safety problem,
    /// but it silently loses usable physical memory.
    ///
    /// KNOWN FAILURE — the same too-narrow mask as the soundness direction
    /// above, seen from the other side: a three-bit mask also *matches* runs
    /// whose fourth bit is set but whose test should have failed, so the
    /// offset it reports need not be the free run this harness found.
    #[kani::proof]
    #[kani::unwind(9)]
    fn find_bit_group_does_not_miss_an_available_run() {
        let byte: u8 = kani::any();
        let len: usize = kani::any();
        kani::assume(len == 1 || len == 2 || len == 4);

        // Does *some* aligned run of `len` set bits exist?
        let mut exists = false;
        let mut offset = 0;
        while offset + len <= 8 {
            let mut all_set = true;
            for i in 0..len {
                if byte & (1u8 << (offset + i)) == 0 {
                    all_set = false;
                }
            }
            if all_set {
                exists = true;
            }
            offset += len;
        }

        assert_eq!(find_bit_group(byte, len).is_some(), exists);
    }

    // ---------------------------------------------------------------------------
    // Bit-run helpers used to fill the bitmap from the memory map
    // ---------------------------------------------------------------------------

    /// `set_bit_range` is the whole bit-shape vocabulary `mark_frames_free`
    /// has, so its correctness is exactly "a frame's bit is set iff that frame
    /// is in the range". Proved bit by bit over every legal `(lo, hi)`.
    #[kani::proof]
    #[kani::unwind(9)]
    fn set_bit_range_sets_exactly_that_range() {
        let lo: u8 = kani::any();
        let hi: u8 = kani::any();
        kani::assume(lo <= hi && hi <= 8);

        let byte = set_bit_range(lo, hi);

        for bit in 0..8u8 {
            assert_eq!(
                byte & (1 << bit) != 0,
                bit >= lo && bit < hi,
                "bit is set exactly when it lies in [lo, hi)"
            );
        }
    }

    #[kani::proof]
    fn set_least_significant_bits_is_a_low_anchored_run() {
        let n: u8 = kani::any();
        kani::assume(n <= 8);

        let byte = set_least_significant_bits(n);

        assert_eq!(byte.count_ones(), n as u32, "exactly n bits set");
        assert_eq!(byte.trailing_ones(), n as u32, "and they are the lowest n");
    }

    // ---------------------------------------------------------------------------
    // BitmapFrameAllocator — the unsafe trait's invariants
    // ---------------------------------------------------------------------------

    /// The core soundness property of the whole allocator, over a fully symbolic
    /// bitmap: **every frame `allocate_range` hands out was marked free before the
    /// call, and is marked used after it.**
    ///
    /// A violation here is a physical frame handed to two owners. This harness
    /// covers the `size < 8` path (orders 0..=2), which is the one `allocate()`
    /// and `HeapProvider` actually drive.
    ///
    /// KNOWN FAILURE at `order == 2` — see `docs/kani-findings.md`. The
    /// counterexample is inherited from `find_bit_group`'s mask bug above: a byte
    /// such as `0b0111_0111` makes `find_bit_group(byte, 4)` report offset 0,
    /// after which `allocate_range` clears all four low bits — including bit 3,
    /// which was already allocated. The returned `FrameRange` therefore contains
    /// a frame that is simultaneously owned by a previous allocation.
    #[kani::proof]
    #[kani::unwind(17)]
    fn allocate_range_never_hands_out_a_used_frame() {
        let mut bitmap = [0u8; BITMAP_BYTES];
        for b in bitmap.iter_mut() {
            *b = kani::any();
        }
        let before = bitmap;

        let order: usize = kani::any();
        kani::assume(order <= 2); // size = 1, 2 or 4: the sub-byte path

        // SAFETY: `bitmap` is a local array modelling nothing real, so its "free"
        // bits cannot conflict with any other owner.
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };
        let result = allocator.allocate_range(order);
        drop(allocator);

        if let Some(range) = result {
            let size = 1u64 << order;
            assert_eq!(range.count(), size, "the range is the requested size");

            let first = range.first().index();
            assert!(
                first % size == 0,
                "the range is aligned to its size, as allocate_range documents"
            );
            assert!(
                first + size <= (BITMAP_BYTES * 8) as u64,
                "the range lies within the bitmap"
            );

            for i in 0..size {
                let index = (first + i) as usize;
                assert!(
                    bit_is_free(&before, index),
                    "allocate_range returned a frame that was already in use"
                );
                assert!(
                    !bit_is_free(&bitmap, index),
                    "an allocated frame must be marked used afterwards"
                );
            }

            // Nothing outside the returned range may change state — an allocator
            // that silently freed a neighbouring frame would be just as unsound.
            for index in 0..BITMAP_BYTES * 8 {
                let inside = index as u64 >= first && (index as u64) < first + size;
                if !inside {
                    assert_eq!(
                        bit_is_free(&before, index),
                        bit_is_free(&bitmap, index),
                        "allocate_range disturbed a frame outside the returned range"
                    );
                }
            }
        } else {
            // A refusal must leave the bitmap untouched.
            assert_eq!(before, bitmap);
        }
    }

    /// The whole-byte path (`size >= 8`, i.e. order >= 3), which takes a
    /// completely separate branch: it scans for runs of `u8::MAX` bytes rather
    /// than searching within a byte.
    ///
    /// KNOWN FAILURE — see `docs/kani-findings.md` ("allocate_range unreachable").
    /// When no aligned run of free bytes exists *and* the bitmap length is an
    /// exact multiple of the run length, the `'outer` loop runs to completion and
    /// falls through to `unreachable!()`, panicking instead of returning `None`.
    /// The `return None` inside the loop only fires for a trailing *partial*
    /// chunk, so it cannot cover the exact-multiple case.
    ///
    /// `ORDER` is a const parameter rather than a symbolic value on purpose: a
    /// symbolic order makes `byte_len` — and hence the `step_by` stride of the
    /// `'outer` loop — symbolic, which CBMC cannot unwind in useful time. Each
    /// order gets its own harness below instead.
    fn check_large_allocation_is_sound<const ORDER: usize>() {
        let mut bitmap = [0u8; BITMAP_BYTES];
        for b in bitmap.iter_mut() {
            *b = kani::any();
        }
        let before = bitmap;

        // SAFETY: `bitmap` is a local array modelling nothing real, so its "free"
        // bits cannot conflict with any other owner.
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };
        let result = allocator.allocate_range(ORDER);
        drop(allocator);

        if let Some(range) = result {
            let size = 1u64 << ORDER;
            assert_eq!(range.count(), size);
            let first = range.first().index();
            assert!(first % size == 0, "aligned to its size");
            assert!(first + size <= (BITMAP_BYTES * 8) as u64);

            for i in 0..size {
                let index = (first + i) as usize;
                assert!(
                    bit_is_free(&before, index),
                    "allocate_range returned a frame that was already in use"
                );
                assert!(!bit_is_free(&bitmap, index));
            }
        } else {
            assert_eq!(before, bitmap, "a refusal must leave the bitmap untouched");
        }
    }

    /// Order 3 — one whole bitmap byte (8 frames, 32 KiB).
    #[kani::proof]
    #[kani::unwind(17)]
    fn allocate_range_one_byte_never_hands_out_a_used_frame() {
        check_large_allocation_is_sound::<3>();
    }

    /// Order 4 — two whole bitmap bytes (16 frames, 64 KiB), i.e. the entire
    /// modelled bitmap. This is the case where `bitmap.len()` is an exact multiple
    /// of `byte_len`, which is precisely when the `unreachable!()` fall-through
    /// bites.
    #[kani::proof]
    #[kani::unwind(17)]
    fn allocate_range_two_bytes_never_hands_out_a_used_frame() {
        check_large_allocation_is_sound::<4>();
    }

    /// `reserve` must fail rather than succeed on a frame that is already
    /// allocated or reserved — the second of `FrameAllocator`'s two documented
    /// invariants — and must leave the bitmap untouched when it fails.
    #[kani::proof]
    fn reserve_fails_on_an_unavailable_frame() {
        let mut bitmap = [0u8; BITMAP_BYTES];
        for b in bitmap.iter_mut() {
            *b = kani::any();
        }
        let before = bitmap;

        let index: usize = kani::any();
        kani::assume(index < BITMAP_BYTES * 8);
        let was_free = bit_is_free(&before, index);
        let frame = Frame::new(PhysAddress::from_raw(index as u64 * PAGE_SIZE.as_raw()));

        // SAFETY: as above — a local array modelling nothing real.
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };
        let result = allocator.reserve(frame);
        drop(allocator);

        assert_eq!(
            result.is_ok(),
            was_free,
            "reserve succeeds exactly on frames that were free"
        );

        if result.is_ok() {
            assert!(!bit_is_free(&bitmap, index), "a reserved frame is marked used");
        } else {
            assert_eq!(before, bitmap, "a failed reserve changes nothing");
        }

        // Only the target frame's bit may move.
        for i in 0..BITMAP_BYTES * 8 {
            if i != index {
                assert_eq!(bit_is_free(&before, i), bit_is_free(&bitmap, i));
            }
        }
    }

    /// Reserving a frame must actually keep it out of circulation: after a
    /// successful `reserve`, no subsequent `allocate` may return it. This composes
    /// the two invariants rather than checking them in isolation, which is where
    /// an allocator is most likely to go wrong.
    #[kani::proof]
    #[kani::unwind(17)]
    fn a_reserved_frame_is_never_allocated() {
        let mut bitmap = [0u8; BITMAP_BYTES];
        for b in bitmap.iter_mut() {
            *b = kani::any();
        }

        let index: usize = kani::any();
        kani::assume(index < BITMAP_BYTES * 8);
        kani::assume(bit_is_free(&bitmap, index));
        let frame = Frame::new(PhysAddress::from_raw(index as u64 * PAGE_SIZE.as_raw()));

        // SAFETY: as above — a local array modelling nothing real.
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };
        allocator.reserve(frame).unwrap();

        // Drain every frame the allocator is willing to give up.
        for _ in 0..BITMAP_BYTES * 8 {
            match allocator.allocate() {
                Some(got) => assert_ne!(got, frame, "a reserved frame was allocated"),
                None => break,
            }
        }
    }

    /// `deallocate` asserts the bit is currently *used* before setting it, so a
    /// double-free panics rather than silently marking a live frame free twice —
    /// which would then let the allocator hand the same frame to two owners.
    /// Proved over an arbitrary bitmap and an arbitrary frame index.
    #[kani::proof]
    #[kani::should_panic]
    fn deallocating_a_free_frame_panics_rather_than_double_freeing() {
        let mut bitmap = [0u8; BITMAP_BYTES];
        for b in bitmap.iter_mut() {
            *b = kani::any();
        }

        let index: usize = kani::any();
        kani::assume(index < BITMAP_BYTES * 8);
        // The frame is *already* free: deallocating it again is the double-free.
        kani::assume(bit_is_free(&bitmap, index));

        // SAFETY: as above — a local array modelling nothing real. The call
        // deliberately violates `deallocate`'s documented precondition (the frame
        // was never allocated), which is exactly what this harness asserts is
        // caught.
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };
        unsafe {
            allocator.deallocate(Frame::new(PhysAddress::from_raw(
                index as u64 * PAGE_SIZE.as_raw(),
            )));
        }
    }

    /// Allocate-then-deallocate returns the bitmap to its exact starting state,
    /// for any starting bitmap. Round-tripping is what makes frame reuse safe: a
    /// deallocation that set the wrong bit would free a frame that is still live.
    #[kani::proof]
    #[kani::unwind(17)]
    fn allocate_then_deallocate_restores_the_bitmap() {
        let mut bitmap = [0u8; BITMAP_BYTES];
        for b in bitmap.iter_mut() {
            *b = kani::any();
        }
        let before = bitmap;

        // SAFETY: as above — a local array modelling nothing real.
        let mut allocator = unsafe { BitmapFrameAllocator::new(&mut bitmap) };
        if let Some(frame) = allocator.allocate() {
            // SAFETY: `frame` was just returned by `allocate` and has not been
            // deallocated since, satisfying `deallocate`'s contract.
            unsafe {
                allocator.deallocate(frame);
            }
        }
        drop(allocator);

        assert_eq!(before, bitmap);
    }

    // ---------------------------------------------------------------------------
    // Frame <-> bitmap offset conversion
    // ---------------------------------------------------------------------------

    /// `offsets_to_frame` and `frame_to_offsets` are inverses. Every bitmap
    /// operation converts in one direction and asserts in the other, so a mismatch
    /// would corrupt the free/used state of a *different* frame than the one being
    /// operated on — the worst possible failure mode for an allocator.
    #[kani::proof]
    fn frame_and_bitmap_offsets_are_inverses() {
        let byte_offset: usize = kani::any();
        let bit_offset: u32 = kani::any();
        kani::assume(bit_offset < 8);
        // Keep the frame address representable: `offsets_to_frame` multiplies by
        // 8 * PAGE_SIZE.
        kani::assume(byte_offset < (1 << 40));

        let frame = BitmapFrameAllocator::offsets_to_frame(byte_offset, bit_offset);
        let (back_byte, back_bit) = BitmapFrameAllocator::frame_to_offsets(frame);

        assert_eq!(back_byte, byte_offset);
        assert_eq!(back_bit, bit_offset);
        // And the frame index is the flat bit position, which is the property the
        // bitmap's whole addressing scheme rests on.
        assert_eq!(frame.index(), byte_offset as u64 * 8 + bit_offset as u64);
    }

    // ---------------------------------------------------------------------------
    // BumpFrameAllocator
    // ---------------------------------------------------------------------------

    /// The bootstrap allocator (`mm::init` uses it for the kernel's own page
    /// tables) must never hand out the same frame twice, and must account for
    /// every frame it was given: allocated + remaining is invariant.
    #[kani::proof]
    #[kani::unwind(6)]
    fn bump_allocator_hands_out_each_frame_once() {
        let start: u64 = kani::any();
        let count: u64 = kani::any();
        kani::assume(start & (PAGE_SIZE.as_raw() - 1) == 0);
        kani::assume(count >= 1 && count <= 4);
        kani::assume(start < (1 << 40));

        let range = FrameRange::new(Frame::new(PhysAddress::from_raw(start)), count).unwrap();
        let mut alloc = BumpFrameAllocator::new(range);

        let mut previous: Option<Frame> = None;
        let mut handed_out = 0u64;
        for _ in 0..4 {
            let Some(frame) = alloc.allocate() else { break };
            // Strictly increasing means never repeated.
            if let Some(p) = previous {
                assert!(frame > p, "the bump allocator must never go backwards");
            }
            assert!(frame.index() >= range.first().index());
            assert!(frame.index() <= range.last().index());
            previous = Some(frame);
            handed_out += 1;
        }

        let remaining = alloc.unwrap().map_or(0, |r| r.count());
        assert_eq!(
            handed_out + remaining,
            count,
            "no frame is lost or duplicated across the split"
        );
    }

    // -----------------------------------------------------------------------
    // Building the allocator's initial state
    // -----------------------------------------------------------------------

    /// `mark_frames_free` is the whole of `fill_bitmap_from_map`'s bit
    /// arithmetic, factored out so it can be proved without a `Map` in the
    /// picture. The starting bitmap is symbolic, not zeroed, so this proves
    /// two things at once: every frame in the range ends up free, and every
    /// bit outside it is left exactly as it was.
    ///
    /// The second half is the one that matters for soundness. `mm::init` hands
    /// the finished bitmap to `BitmapFrameAllocator::new`, whose `unsafe`
    /// contract is "all frames marked free must be available for use and not
    /// used by other code" — so a bit set outside the `Available` range is
    /// firmware memory, kernel image, or MMIO that the allocator will later
    /// hand out as ordinary RAM. The three-way split this replaced did exactly
    /// that for a range confined to one bitmap byte.
    #[kani::proof]
    #[kani::unwind(20)]
    fn mark_frames_free_marks_exactly_its_frames() {
        const MODELLED_FRAMES: u64 = (BITMAP_BYTES * 8) as u64;

        let first: u64 = kani::any();
        let count: u64 = kani::any();
        // Bound each operand *before* constraining the sum: `kani::assume`
        // evaluates its argument, so `first + count <= N` on unconstrained
        // `u64`s overflows on the way into the assumption.
        kani::assume(first <= MODELLED_FRAMES);
        kani::assume(count >= 1 && count <= MODELLED_FRAMES);
        kani::assume(first + count <= MODELLED_FRAMES);

        let mut bitmap = [0u8; BITMAP_BYTES];
        for b in bitmap.iter_mut() {
            *b = kani::any();
        }
        let before = bitmap;

        let frames = FrameRange::new(
            Frame::new(PhysAddress::from_raw(first * PAGE_SIZE.as_raw())),
            count,
        )
        .unwrap();

        mark_frames_free(&mut bitmap, frames);

        for index in 0..MODELLED_FRAMES as usize {
            if index as u64 >= first && (index as u64) < first + count {
                assert!(
                    bit_is_free(&bitmap, index),
                    "every frame in the range is marked free"
                );
            } else {
                assert_eq!(
                    bit_is_free(&bitmap, index),
                    bit_is_free(&before, index),
                    "a frame outside the range must be left untouched"
                );
            }
        }
    }

    /// End to end: from a memory map to the bitmap `mm::init` hands the
    /// allocator, exactly the `Available` frames come back free.
    ///
    /// This became affordable once `Map::iter_type` was fixed to scan the
    /// `num_entries` prefix instead of all 128 backing slots — the harness
    /// previously had to unwind two 128-iteration loops and did not settle in
    /// ten minutes.
    #[kani::proof]
    #[kani::unwind(20)]
    fn fill_bitmap_marks_exactly_the_available_frames() {
        use crate::memory::{Map, MapEntry, MemoryType};

        const MODELLED_FRAMES: u64 = (BITMAP_BYTES * 8) as u64;

        // A single symbolic `Available` region, expressed in whole frames so
        // it survives `iter_map_frames`' shrink-to-alignment unchanged.
        let first: u64 = kani::any();
        let count: u64 = kani::any();
        // Bound each operand *before* constraining the sum: `kani::assume`
        // evaluates its argument, so `first + count <= N` on unconstrained
        // `u64`s overflows on the way into the assumption.
        kani::assume(first <= MODELLED_FRAMES);
        kani::assume(count >= 1 && count <= MODELLED_FRAMES);
        kani::assume(first + count <= MODELLED_FRAMES);

        let map = Map::from_entries([MapEntry {
            extent: PhysExtent::from_raw(
                first * PAGE_SIZE.as_raw(),
                count * PAGE_SIZE.as_raw(),
            ),
            mem_type: MemoryType::Available,
        }]);

        let mut bitmap = [0u8; BITMAP_BYTES];
        fill_bitmap_from_map(&mut bitmap, &map);

        for index in 0..MODELLED_FRAMES as usize {
            assert_eq!(
                bit_is_free(&bitmap, index),
                index as u64 >= first && (index as u64) < first + count,
                "a frame is free exactly when it lies in an Available region"
            );
        }
    }
}
