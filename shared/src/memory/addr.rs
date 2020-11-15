use core::cmp::{max, min};
use core::fmt::Debug;
use core::hash::Hash;
use core::marker::PhantomData;

pub trait AddressType: Clone + Copy + Eq + Ord + PartialEq + PartialOrd + Debug + Hash {}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
pub struct PhysAddressType;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
pub struct VirtAddressType;

impl AddressType for PhysAddressType {}
impl AddressType for VirtAddressType {}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
pub struct Address<Type: AddressType>(u64, PhantomData<Type>);

pub type PhysAddress = Address<PhysAddressType>;
pub type VirtAddress = Address<VirtAddressType>;

impl<Type: AddressType> Address<Type> {
    pub fn from_raw(val: u64) -> Self {
        Self(val, PhantomData)
    }

    pub fn zero() -> Self {
        Self::from_raw(0)
    }

    pub fn as_raw(self) -> u64 {
        self.0
    }

    pub fn from_zero(offset: Length) -> Self {
        Self::offset_by(Self::zero(), offset)
    }

    pub fn distance_from(self, left: Self) -> Length {
        assert!(self >= left);
        Length::from_raw(self.as_raw() - left.as_raw())
    }

    pub fn distance_to(self, right: Self) -> Length {
        assert!(self <= right);
        Length::from_raw(right.as_raw() - self.as_raw())
    }

    pub fn offset_by(self, length: Length) -> Self {
        self.offset_by_checked(length).unwrap()
    }

    pub fn offset_by_checked(self, length: Length) -> Option<Self> {
        if length.as_raw() > u64::MAX - self.as_raw() {
            return None;
        }

        Some(Self::from_raw(self.as_raw() + length.as_raw()))
    }

    pub fn is_aligned_to(self, alignment: u64) -> bool {
        self == self.align_down(alignment)
    }

    /// Returns the last address below `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_down(self, alignment: u64) -> Self {
        Self::from_raw(align_u64_down(self.as_raw(), alignment))
    }

    /// Returns the first address above `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_up(self, alignment: u64) -> Self {
        Self::from_raw(align_u64_up(self.as_raw(), alignment))
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
pub struct Length(u64);

impl Length {
    pub const fn from_raw(val: u64) -> Length {
        Length(val)
    }

    pub const fn as_raw(self) -> u64 {
        self.0
    }

    pub fn add(self, rhs: Length) -> Length {
        Length::from_raw(self.as_raw() + rhs.as_raw())
    }

    pub fn subtract(self, rhs: Length) -> Length {
        assert!(self.as_raw() >= rhs.as_raw());
        Length::from_raw(self.as_raw() - rhs.as_raw())
    }

    pub fn times(self, x: u64) -> Length {
        Self::from_raw(self.as_raw().checked_mul(x).unwrap())
    }

    pub fn is_aligned_to(self, alignment: u64) -> bool {
        self == self.align_down(alignment)
    }

    /// Returns the last length lesser than `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_down(self, alignment: u64) -> Length {
        Length::from_raw(align_u64_down(self.as_raw(), alignment))
    }

    /// Returns the first length greater than `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_up(self, alignment: u64) -> Length {
        Length::from_raw(align_u64_up(self.as_raw(), alignment))
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash)]
pub struct Extent<Type: AddressType> {
    pub address: Address<Type>,
    pub length: Length,
}

pub type PhysExtent = Extent<PhysAddressType>;
pub type VirtExtent = Extent<VirtAddressType>;

impl<Type: AddressType> Extent<Type> {
    pub fn new(address: Address<Type>, length: Length) -> Self {
        Self::new_checked(address, length).unwrap()
    }

    pub fn new_checked(address: Address<Type>, length: Length) -> Option<Self> {
        if length.as_raw() == 0 || length.as_raw() > u64::MAX - address.as_raw() {
            None
        } else {
            Some(Self {
                address: address,
                length: length,
            })
        }
    }

    pub fn from_raw(address: u64, length: u64) -> Self {
        Self::new(Address::<Type>::from_raw(address), Length::from_raw(length))
    }

    pub fn from_raw_range_exclusive(begin_address: u64, end_address: u64) -> Self {
        Self::from_range_exclusive(
            Address::<Type>::from_raw(begin_address),
            Address::<Type>::from_raw(end_address),
        )
    }

    pub fn from_range_exclusive(begin: Address<Type>, end: Address<Type>) -> Self {
        Self {
            address: begin,
            length: begin.distance_to(end),
        }
    }

    pub fn address(self) -> Address<Type> {
        self.address
    }

    pub fn length(self) -> Length {
        self.length
    }

    /// The first address just outside us, to the right
    pub fn end_address(self) -> Address<Type> {
        self.address.offset_by(self.length)
    }

    /// The last address in the extent. E.g.
    ///
    ///
    /// ```
    /// use shared::memory::addr::*;
    /// assert_eq!(PhysExtent::from_raw(0, 4).last_address(), PhysAddress::from_raw(3));
    /// ```
    pub fn last_address(self) -> Address<Type> {
        self.address
            .offset_by(self.length.subtract(Length::from_raw(1)))
    }

    pub fn overlap(self, other: Self) -> Option<Self> {
        if self.address > other.address {
            return other.overlap(self);
        }

        let overlap_start = other.address;

        if self.address.distance_to(overlap_start) >= self.length {
            return None;
        }

        let overlap_length = min(
            self.length
                .subtract(self.address.distance_to(overlap_start)),
            other.length,
        );

        Some(Self {
            address: overlap_start,
            length: overlap_length,
        })
    }

    pub fn has_overlap(self, other: Self) -> bool {
        self.overlap(other).is_some()
    }

    pub fn left_difference(self, other: Self) -> Option<Self> {
        if self.address >= other.address {
            return None;
        }

        // Since our address is strictly less than `other`'s, we can safely
        // assume the result is non-empty.
        let diff_length = min(self.length, self.address.distance_to(other.address));

        Some(Self {
            address: self.address,
            length: diff_length,
        })
    }

    pub fn right_difference(self, other: Self) -> Option<Self> {
        if self.last_address() <= other.last_address() {
            return None;
        }

        // Since our right endpoint is completely to the left `other`, the right
        // difference is non-empty. Additionally, since `self.end_address() <=
        // u64::MAX + 1`, we can be assured that `other.end_address() <=
        // u64::MAX`.

        let diff_address = max(self.address, other.end_address());
        let diff_length = self
            .length
            .subtract(diff_address.distance_from(self.address));

        Some(Self {
            address: diff_address,
            length: diff_length,
        })
    }

    pub fn is_aligned_to(self, alignment: u64) -> bool {
        self.address.is_aligned_to(alignment) && self.length.is_aligned_to(alignment)
    }

    /// Returns the largest extent completely contained in `self` whose start
    /// and end addresses are aligned to `alignment`. `alignment` must be a
    /// power of two.
    pub fn shrink_to_alignment(self, alignment: u64) -> Option<Self> {
        let start_address = self.address.align_up(alignment);
        let end_address = self.end_address().align_down(alignment);
        if end_address <= start_address {
            None
        } else {
            Some(Self {
                address: start_address,
                length: start_address.distance_to(end_address),
            })
        }
    }

    /// Returns the smallest extent that contains `self` whose start and end
    /// addresses are aligned to `alignment`. `alignment` must be a power of
    /// two. There is always a valid result.
    pub fn expand_to_alignment(&self, alignment: u64) -> Self {
        let start_address = self.address.align_down(alignment);
        let end_address = self.end_address().align_up(alignment);

        // TODO: handle if `end_address` extends beyond u64::MAX
        Self {
            address: start_address,
            length: start_address.distance_to(end_address),
        }
    }
}

/// Given power-of-two `alignment`, returns the largest value below `x` aligned
/// to `alignment`
const fn align_u64_down(x: u64, alignment: u64) -> u64 {
    let mask = !(alignment - 1);
    x & mask
}

/// Given power-of-two `alignment`, returns the smallest value above `x` aligned
/// to `alignment`
const fn align_u64_up(x: u64, alignment: u64) -> u64 {
    align_u64_down(x + (alignment - 1), alignment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_raw() {
        assert_eq!(align_u64_down(0, 2), 0);
        assert_eq!(align_u64_down(1, 2), 0);
        assert_eq!(align_u64_down(2, 2), 2);

        assert_eq!(align_u64_up(0, 2), 0);
        assert_eq!(align_u64_up(1, 2), 2);
        assert_eq!(align_u64_up(2, 2), 2);

        assert_eq!(align_u64_down(255, 1024), 0);
        assert_eq!(align_u64_up(255, 1024), 1024);
    }

    #[test]
    fn align_address() {
        assert_eq!(
            PhysAddress::from_raw(0).align_down(1024),
            PhysAddress::from_raw(0)
        );
        assert_eq!(
            PhysAddress::from_raw(0).align_up(1024),
            PhysAddress::from_raw(0)
        );

        assert_eq!(
            PhysAddress::from_raw(1024).align_down(1024),
            PhysAddress::from_raw(1024)
        );
        assert_eq!(
            PhysAddress::from_raw(1024).align_up(1024),
            PhysAddress::from_raw(1024)
        );

        assert_eq!(
            PhysAddress::from_raw(1).align_down(1024),
            PhysAddress::from_raw(0)
        );
        assert_eq!(
            PhysAddress::from_raw(1).align_up(1024),
            PhysAddress::from_raw(1024)
        );

        assert_eq!(
            PhysAddress::from_raw(1023).align_down(1024),
            PhysAddress::from_raw(0)
        );
        assert_eq!(
            PhysAddress::from_raw(1023).align_up(1024),
            PhysAddress::from_raw(1024)
        );
    }

    #[test]
    fn overlap_extent() {
        assert_eq!(
            PhysExtent::from_raw(0, 8).overlap(PhysExtent::from_raw(0, 8)),
            Some(PhysExtent::from_raw(0, 8))
        );

        assert_eq!(
            PhysExtent::from_raw(0, 8).overlap(PhysExtent::from_raw(8, 8)),
            None
        );
        assert_eq!(
            PhysExtent::from_raw(0, 8).overlap(PhysExtent::from_raw(1024, 8)),
            None
        );

        assert_eq!(
            PhysExtent::from_raw(5, 5).overlap(PhysExtent::from_raw(8, 7)),
            Some(PhysExtent::from_raw(8, 2))
        );
        assert_eq!(
            PhysExtent::from_raw(8, 7).overlap(PhysExtent::from_raw(5, 5)),
            Some(PhysExtent::from_raw(8, 2))
        );

        assert_eq!(
            PhysExtent::from_raw(0, 10).overlap(PhysExtent::from_raw(2, 3)),
            Some(PhysExtent::from_raw(2, 3))
        );
        assert_eq!(
            PhysExtent::from_raw(2, 3).overlap(PhysExtent::from_raw(0, 10)),
            Some(PhysExtent::from_raw(2, 3))
        );
    }

    #[test]
    fn shrink_extent() {
        let extent = PhysExtent::from_raw(1, 8191)
            .shrink_to_alignment(4096)
            .unwrap();
        assert_eq!(extent, PhysExtent::from_raw(4096, 4096));

        let extent = PhysExtent::from_raw(0, 4097)
            .shrink_to_alignment(4096)
            .unwrap();
        assert_eq!(extent, PhysExtent::from_raw(0, 4096));

        let extent = PhysExtent::from_raw(4095, 4097)
            .shrink_to_alignment(4096)
            .unwrap();
        assert_eq!(extent, PhysExtent::from_raw(4096, 4096));
    }

    #[test]
    fn shrink_extent_already_aligned() {
        // An already-aligned extent should not be shrunk.
        let extent = PhysExtent::from_raw(0, 4096);
        assert_eq!(extent, extent.shrink_to_alignment(4096).unwrap());

        let extent = PhysExtent::from_raw(4096, 8192);
        assert_eq!(extent, extent.shrink_to_alignment(4096).unwrap());
    }

    #[test]
    fn shrink_extent_empty() {
        // If there's no aligned sub-extent, it must return None.
        let extent = PhysExtent::from_raw(1, 4096).shrink_to_alignment(4096);
        assert_eq!(extent, None);

        let extent = PhysExtent::from_raw(0, 4095).shrink_to_alignment(4096);
        assert_eq!(extent, None);

        let extent = PhysExtent::from_raw(1, 8190).shrink_to_alignment(4096);
        assert_eq!(extent, None);
    }

    #[test]
    fn left_difference() {
        assert_eq!(
            PhysExtent::from_raw(10, 10).left_difference(PhysExtent::from_raw(0, 10)),
            None
        );
        assert_eq!(
            PhysExtent::from_raw(10, 10).left_difference(PhysExtent::from_raw(10, 10)),
            None
        );
        assert_eq!(
            PhysExtent::from_raw(10, 10).left_difference(PhysExtent::from_raw(20, 10)),
            Some(PhysExtent::from_raw(10, 10))
        );

        assert_eq!(
            PhysExtent::from_raw(10, 10).left_difference(PhysExtent::from_raw(5, 10)),
            None
        );
        assert_eq!(
            PhysExtent::from_raw(10, 10).left_difference(PhysExtent::from_raw(15, 10)),
            Some(PhysExtent::from_raw(10, 5))
        );

        assert_eq!(
            PhysExtent::from_raw(10, 10).left_difference(PhysExtent::from_raw(12, 6)),
            Some(PhysExtent::from_raw(10, 2))
        );

        assert_eq!(
            PhysExtent::from_raw(10, 10).left_difference(PhysExtent::from_raw(8, 14)),
            None
        );
    }

    #[test]
    fn right_difference() {
        assert_eq!(
            PhysExtent::from_raw(10, 10).right_difference(PhysExtent::from_raw(0, 10)),
            Some(PhysExtent::from_raw(10, 10))
        );
        assert_eq!(
            PhysExtent::from_raw(10, 10).right_difference(PhysExtent::from_raw(10, 10)),
            None
        );
        assert_eq!(
            PhysExtent::from_raw(10, 10).right_difference(PhysExtent::from_raw(20, 10)),
            None
        );

        assert_eq!(
            PhysExtent::from_raw(10, 10).right_difference(PhysExtent::from_raw(5, 10)),
            Some(PhysExtent::from_raw(15, 5))
        );
        assert_eq!(
            PhysExtent::from_raw(10, 10).right_difference(PhysExtent::from_raw(15, 10)),
            None
        );

        assert_eq!(
            PhysExtent::from_raw(10, 10).right_difference(PhysExtent::from_raw(12, 6)),
            Some(PhysExtent::from_raw(18, 2))
        );

        assert_eq!(
            PhysExtent::from_raw(10, 10).right_difference(PhysExtent::from_raw(8, 14)),
            None
        );
    }
}
