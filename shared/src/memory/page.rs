//! Data structures representing pages and frames

use super::addr::{Length, PhysAddress, PhysExtent, VirtAddress, VirtExtent};

pub const PAGE_SIZE: Length = Length::from_raw(4096);

/// A 4 KiB physical memory frame
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Frame {
    start: PhysAddress,
}

impl Frame {
    /// Creates a `Frame` representing the frame beginning at `start`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is not aligned to `PAGE_SIZE`.
    pub fn new(start: PhysAddress) -> Frame {
        assert!(start.is_aligned_to(PAGE_SIZE.as_raw()));
        Frame { start }
    }

    /// Gets the `Frame` that contains `addr`.
    pub fn containing(addr: PhysAddress) -> Frame {
        Self::new(addr.align_down(PAGE_SIZE.as_raw()))
    }

    /// Start address of the frame
    pub fn start(self) -> PhysAddress {
        self.start
    }

    /// Extent of memory contained in the frame
    pub fn extent(self) -> PhysExtent {
        PhysExtent::new(self.start, PAGE_SIZE)
    }

    /// The nth frame after `self`, or `None` if it's not addressable
    pub fn next(self, n: u64) -> Option<Frame> {
        let next_start = self
            .start
            .offset_by_checked(Length::from_raw(PAGE_SIZE.as_raw().checked_mul(n)?))?;
        Some(Self::new(next_start))
    }
}

/// A 4 KiB virtual memory page
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page {
    start: VirtAddress,
}

impl Page {
    /// Creates a `Page` representing the page beginning at `start`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is not aligned to `PAGE_SIZE`.
    pub fn new(start: VirtAddress) -> Page {
        assert!(start.is_aligned_to(PAGE_SIZE.as_raw()));
        Page { start }
    }

    /// Gets the `Page` that contains `addr`.
    pub fn containing(addr: VirtAddress) -> Page {
        Self::new(addr.align_down(PAGE_SIZE.as_raw()))
    }

    /// Start address of the page
    pub fn start(&self) -> VirtAddress {
        self.start
    }

    /// Extent of virtual address space contained in the page
    pub fn extent(&self) -> VirtExtent {
        VirtExtent::new(self.start, PAGE_SIZE)
    }

    /// The nth page after `self`, or `None` if it's not addressable
    pub fn next(self, n: u64) -> Option<Page> {
        let next_start = self
            .start
            .offset_by_checked(Length::from_raw(PAGE_SIZE.as_raw().checked_mul(n)?))?;
        Some(Self::new(next_start))
    }
}

/// A contiguous range of physical memory frames. Always non-empty.
pub struct FrameRange {
    first: Frame,
    count: u64,
}

impl FrameRange {
    pub fn new(first: Frame, count: u64) -> Option<FrameRange> {
        if count == 0 {
            return None;
        }

        // Check that `count` frames after and including `first` are
        // addressable. `first.next(count)` may not be addressable if the range
        // includes the last frame.
        if first.next(count - 1).is_none() {
            return None;
        }

        Some(FrameRange { first, count })
    }

    // All frames between and including `first` to `last`
    pub fn between_inclusive(first: Frame, last: Frame) -> FrameRange {
        let len = last.start().distance_from(first.start());
        let count = len.as_raw() / PAGE_SIZE.as_raw();
        FrameRange { first, count }
    }

    // All frames between `first` to `last`, including `first` but not `last`
    pub fn between_exclusive(first: Frame, last: Frame) -> FrameRange {
        let len = last.start().distance_from(first.start());
        let count = len.as_raw() / PAGE_SIZE.as_raw() + 1;
        FrameRange { first, count }
    }

    pub fn first(&self) -> Frame {
        self.first
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    // The last `Frame` within the range
    pub fn last(&self) -> Frame {
        self.first.next(self.count - 1).unwrap()
    }

    // The first `Frame` after the range, or `None` if it ends at the last frame.
    pub fn end(&self) -> Option<Frame> {
        self.first.next(self.count)
    }
}

/// A contiguous range of virtual memory pages. Always non-empty.
pub struct PageRange {
    first: Page,
    count: u64,
}

impl PageRange {
    pub fn new(first: Page, count: u64) -> Option<PageRange> {
        if count == 0 {
            return None;
        }

        // Check that `count` frames after and including `first` are
        // addressable. `first.next(count)` may not be addressable if the range
        // includes the last frame.
        if first.next(count - 1).is_none() {
            return None;
        }

        Some(PageRange { first, count })
    }

    // All frames between and including `first` to `last`
    pub fn between_inclusive(first: Page, last: Page) -> PageRange {
        let len = last.start().distance_from(first.start());
        let count = len.as_raw() / PAGE_SIZE.as_raw();
        PageRange { first, count }
    }

    // All frames between `first` to `last`, including `first` but not `last`
    pub fn between_exclusive(first: Page, last: Page) -> PageRange {
        let len = last.start().distance_from(first.start());
        let count = len.as_raw() / PAGE_SIZE.as_raw() + 1;
        PageRange { first, count }
    }

    pub fn first(&self) -> Page {
        self.first
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    // The last `Page` within the range
    pub fn last(&self) -> Page {
        self.first.next(self.count - 1).unwrap()
    }

    // The first `Page` after the range, or `None` if it ends at the last frame.
    pub fn end(&self) -> Option<Page> {
        self.first.next(self.count)
    }
}
