use core::cmp::{max, min};
use core::convert::Into;
use core::fmt::Debug;
use core::hash::Hash;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign};

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
    pub const fn from_raw(val: u64) -> Self {
        Self(val, PhantomData)
    }

    pub const fn zero() -> Self {
        Self::from_raw(0)
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub const fn as_raw(self) -> u64 {
        self.0
    }

    pub const fn from_zero(offset: Length) -> Self {
        Self::from_raw(offset.as_raw())
    }

    pub fn offset_by_checked(self, length: Length) -> Option<Self> {
        Some(Self(self.0.checked_add(length.0)?, PhantomData))
    }

    pub const fn is_aligned_to(self, alignment: u64) -> bool {
        self.0 == self.align_down(alignment).0
    }

    pub const fn is_aligned_to_length(self, alignment: Length) -> bool {
        self.is_aligned_to(alignment.0)
    }

    /// Returns the last address below `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub const fn align_down(self, alignment: u64) -> Self {
        Self::from_raw(align_u64_down(self.as_raw(), alignment))
    }

    /// Returns the first address above `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub const fn align_up(self, alignment: u64) -> Self {
        Self::from_raw(align_u64_up(self.as_raw(), alignment))
    }
}

impl<Type: AddressType> Add<Length> for Address<Type> {
    type Output = Self;
    fn add(self, rhs: Length) -> Self {
        self.offset_by_checked(rhs).unwrap()
    }
}

impl<Type: AddressType> AddAssign<Length> for Address<Type> {
    fn add_assign(&mut self, rhs: Length) {
        *self = *self + rhs;
    }
}

impl<Type: AddressType> Sub<Length> for Address<Type> {
    type Output = Self;
    fn sub(self, rhs: Length) -> Self {
        Self(self.0.checked_sub(rhs.0).unwrap(), PhantomData)
    }
}

impl<Type: AddressType> SubAssign<Length> for Address<Type> {
    fn sub_assign(&mut self, rhs: Length) {
        *self = *self - rhs;
    }
}

impl<Type: AddressType> Sub<Self> for Address<Type> {
    type Output = Length;
    fn sub(self, rhs: Self) -> Length {
        Length(self.0.checked_sub(rhs.0).unwrap())
    }
}

impl Address<VirtAddressType> {
    pub fn from_ptr<T>(p: *const T) -> Self {
        Self::from_raw(p as usize as u64)
    }

    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as usize as *const _
    }

    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as usize as *mut _
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

    pub const fn is_aligned_to(self, alignment: u64) -> bool {
        self.0 == self.align_down(alignment).0
    }

    /// Returns the last length lesser than `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub const fn align_down(self, alignment: u64) -> Length {
        Length::from_raw(align_u64_down(self.as_raw(), alignment))
    }

    /// Returns the first length greater than `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub const fn align_up(self, alignment: u64) -> Length {
        Length::from_raw(align_u64_up(self.as_raw(), alignment))
    }
}

impl Add for Length {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Length(self.0 + rhs.0)
    }
}

impl AddAssign for Length {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Length {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Length(self.0 - rhs.0)
    }
}

impl SubAssign for Length {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl<Int> Mul<Int> for Length
where
    Int: Into<u64>,
{
    type Output = Self;
    fn mul(self, rhs: Int) -> Self {
        Length(self.0.checked_mul(rhs.into()).unwrap())
    }
}

impl<Int> MulAssign<Int> for Length
where
    Int: Into<u64>,
{
    fn mul_assign(&mut self, rhs: Int) {
        *self = *self * rhs;
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
    pub const fn new(address: Address<Type>, length: Length) -> Self {
        Self::new_checked(address, length).unwrap()
    }

    pub const fn new_checked(address: Address<Type>, length: Length) -> Option<Self> {
        if length.as_raw() == 0 || length.as_raw() > u64::MAX - address.as_raw() {
            None
        } else {
            Some(Self { address, length })
        }
    }

    pub const fn from_raw(address: u64, length: u64) -> Self {
        Self::new(Address::<Type>::from_raw(address), Length::from_raw(length))
    }

    pub const fn from_raw_range_exclusive(begin_address: u64, end_address: u64) -> Self {
        Self::from_range_exclusive(
            Address::<Type>::from_raw(begin_address),
            Address::<Type>::from_raw(end_address),
        )
    }

    pub const fn from_range_exclusive(begin: Address<Type>, end: Address<Type>) -> Self {
        Self {
            address: begin,
            length: Length::from_raw(end.as_raw() - begin.as_raw()),
        }
    }

    pub fn from_range_inclusive(start: Address<Type>, last: Address<Type>) -> Self {
        Self {
            address: start,
            length: (last - start) + Length::from_raw(1),
        }
    }

    pub const fn address(self) -> Address<Type> {
        self.address
    }

    pub const fn length(self) -> Length {
        self.length
    }

    /// The first address just outside us, to the right
    pub fn end_address(self) -> Address<Type> {
        self.address + self.length
    }

    /// The last address in the extent. E.g.
    ///
    ///
    /// ```
    /// use shared::memory::addr::*;
    /// assert_eq!(PhysExtent::from_raw(0, 4).last_address(), PhysAddress::from_raw(3));
    /// ```
    pub fn last_address(self) -> Address<Type> {
        self.address + self.length - Length::from_raw(1)
    }

    pub fn overlap(self, other: Self) -> Option<Self> {
        if self.address > other.address {
            return other.overlap(self);
        }

        let overlap_start = other.address;

        if overlap_start - self.address >= self.length {
            return None;
        }

        let overlap_length = min(self.length - (overlap_start - self.address), other.length);

        Some(Self {
            address: overlap_start,
            length: overlap_length,
        })
    }

    /// Calculate the smallest extent that contains `self` and `other`.
    pub fn join(self, other: Self) -> Self {
        let min_start = min(self.address(), other.address());
        let max_last = max(self.last_address(), other.last_address());
        Self::from_range_inclusive(min_start, max_last)
    }

    pub fn has_overlap(self, other: Self) -> bool {
        self.overlap(other).is_some()
    }

    pub fn contains(self, other: Self) -> bool {
        let Some(overlap) = self.overlap(other) else { return false };
        overlap == other
    }

    pub fn left_difference(self, other: Self) -> Option<Self> {
        if self.address >= other.address {
            return None;
        }

        // Since our address is strictly less than `other`'s, we can safely
        // assume the result is non-empty.
        let diff_length = min(self.length, other.address - self.address);

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
        let diff_length = self.length - (diff_address - self.address);

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
                length: end_address - start_address,
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
            length: end_address - start_address,
        }
    }
}

impl Extent<VirtAddressType> {
    pub fn as_slice<T>(self) -> *const [T] {
        core::ptr::slice_from_raw_parts(self.address().as_ptr(), self.length().as_raw() as usize)
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

    #[test]
    fn contains() {
        assert!(!PhysExtent::from_raw(10, 10).contains(PhysExtent::from_raw(0, 10)));
        assert!(PhysExtent::from_raw(10, 10).contains(PhysExtent::from_raw(10, 10)));
        assert!(!PhysExtent::from_raw(10, 10).contains(PhysExtent::from_raw(20, 10)));

        assert!(!PhysExtent::from_raw(10, 10).contains(PhysExtent::from_raw(5, 10)));

        assert!(PhysExtent::from_raw(0, 10).contains(PhysExtent::from_raw(5, 4)));
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn overlap_is_commutative((a_first, a_last, b_first, b_last) in any::<(u64, u64, u64, u64)>()) {
            prop_assume!(a_first <= a_last);
            prop_assume!(b_first <= b_last);
            let a = PhysExtent::from_range_inclusive(PhysAddress::from_raw(a_first), PhysAddress::from_raw(a_last));
            let b = PhysExtent::from_range_inclusive(PhysAddress::from_raw(b_first), PhysAddress::from_raw(b_last));
            prop_assert_eq!(a.overlap(b), b.overlap(a));
        }
    }
}
