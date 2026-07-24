use core::cmp::{max, min};
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

// `repr(transparent)`: `Address`/`Length`/`Extent` cross the loader/kernel
// boot-info handoff (see `shared::boot_info::BootInfo`) as raw bytes written
// by one independently-compiled binary and read by another, built for
// different targets (`x86_64-unknown-uefi` vs. the custom
// `x86_64-unknown-none.json`). Rust's default repr gives no cross-compilation
// layout guarantee at all, so these need an explicit repr to be sound to
// reinterpret across that boundary.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
#[repr(transparent)]
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
    ///
    /// # Panics
    ///
    /// Panics if there is no such address below `u64::MAX`; use
    /// [`Self::align_up_checked`] where that is reachable.
    pub const fn align_up(self, alignment: u64) -> Self {
        Self::from_raw(align_u64_up(self.as_raw(), alignment))
    }

    /// Like [`Self::align_up`], but returns `None` instead of panicking when
    /// the next aligned address is not representable — that is, when `self`
    /// falls in the last, partial aligned block of the address space.
    pub const fn align_up_checked(self, alignment: u64) -> Option<Self> {
        match align_u64_up_checked(self.as_raw(), alignment) {
            Some(aligned) => Some(Self::from_raw(aligned)),
            None => None,
        }
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

// See the `repr(transparent)` note on `Address` above; same cross-boundary
// reasoning applies here.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
#[repr(transparent)]
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

    /// The minimum number of pages of length at least `self`.
    pub const fn num_pages(self) -> u64 {
        (self.0 - 1 + super::PAGE_SIZE.0) / super::PAGE_SIZE.0
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

// See the `repr(transparent)` note on `Address` above. `Extent` has two
// non-ZST fields, so it needs `repr(C)` (fixed field order/padding) rather
// than `repr(transparent)`.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash)]
#[repr(C)]
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
        let Some(overlap) = self.overlap(other) else {
            return false;
        };
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
    ///
    /// `None` if no such extent exists, including when `self` starts inside
    /// the last partial aligned block of the address space: rounding the start
    /// up then has no representable answer, and there is correspondingly no
    /// aligned sub-extent, since `self`'s end can only be lower still.
    pub fn shrink_to_alignment(self, alignment: u64) -> Option<Self> {
        let start_address = self.address.align_up_checked(alignment)?;
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
/// to `alignment`, or `None` if that value is not representable — i.e. when `x`
/// lies in the last, partial aligned block below `u64::MAX`.
///
/// Rounding up is not a total function on `u64`, so this is the primitive and
/// [`align_u64_up`] is the panicking wrapper. Going through `checked_add` also
/// means a release build (no overflow checks) faults loudly instead of
/// wrapping to a small value, which for a caller computing a mapping boundary
/// is far more dangerous than a panic.
const fn align_u64_up_checked(x: u64, alignment: u64) -> Option<u64> {
    match x.checked_add(alignment - 1) {
        Some(sum) => Some(align_u64_down(sum, alignment)),
        None => None,
    }
}

/// Given power-of-two `alignment`, returns the smallest value above `x` aligned
/// to `alignment`
///
/// # Panics
///
/// Panics if no such value fits in a `u64`; see [`align_u64_up_checked`].
const fn align_u64_up(x: u64, alignment: u64) -> u64 {
    match align_u64_up_checked(x, alignment) {
        Some(aligned) => aligned,
        None => panic!("aligning up would exceed u64::MAX"),
    }
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

#[cfg(kani)]
mod verify {
    //! Kani proof harnesses for [`crate::memory::addr`].
    //!
    //! These are proofs, not tests: every harness below quantifies over the *full*
    //! `u64` domain unless a `kani::assume` narrows it, so a `SUCCESS` means the
    //! property holds for all 2^64 inputs — not just the handful `proptest`
    //! happened to sample. Where a harness does assume something, that assumption
    //! *is* the function's precondition, stated executably; if a caller can
    //! violate it, the assumption marks a real (currently undocumented) contract.
    //!
    //! The recurring technique here is the **probe address**. Rather than compare
    //! two `Extent`s structurally (which only proves the implementation agrees
    //! with itself), each set-like operation is checked against a membership
    //! predicate at a symbolic address `p`. Because `p` is universally quantified,
    //! `contains_addr(result, p) == spec(p)` for all `p` is exactly extensional
    //! equality of the two sets — a specification, independent of how `Extent`
    //! chooses to represent a range.

    use super::*;

    /// Membership predicate: is `a` inside `e`?
    ///
    /// Written as an offset comparison rather than `a >= start && a <= last` so it
    /// can't itself overflow, and so it stays meaningful for an extent whose last
    /// address is `u64::MAX`. This is the *specification* of what an extent means;
    /// none of the code under proof is reused here.
    fn contains_addr<T: AddressType>(e: Extent<T>, a: Address<T>) -> bool {
        a.as_raw() >= e.address().as_raw()
            && (a.as_raw() - e.address().as_raw()) < e.length().as_raw()
    }

    /// A symbolic power-of-two alignment. Kani explores every `1 << k`, so proofs
    /// over this cover all 64 legal alignments rather than the 4 KiB/2 MiB/1 GiB
    /// constants the code happens to use today.
    ///
    /// Reserved for the *primitive* alignment proofs. A symbolic shift amount
    /// makes every bit of the resulting mask a function of `k`, which the solver
    /// handles fine against one arithmetic op but not against a chain of them —
    /// see `any_page_alignment` below.
    fn any_alignment() -> u64 {
        let k: u32 = kani::any();
        kani::assume(k < 64);
        1u64 << k
    }

    /// The three alignments this kernel actually aligns extents to: 4 KiB, 2 MiB
    /// and 1 GiB — the x86-64 page sizes, which are the only values ever passed to
    /// `shrink_to_alignment`/`expand_to_alignment` (see `iter_map_frames` and
    /// `Mapper::map_range`'s phase boundaries).
    ///
    /// Used instead of `any_alignment` for the compound extent proofs. Combining a
    /// symbolic shift with `align_up` + `align_down` + `overlap`'s comparison
    /// chain pushes CBMC past useful runtimes (>15 min on a fully symbolic
    /// alignment), whereas enumerating the three real page sizes keeps the
    /// address operands fully symbolic — the part that matters — and finishes in
    /// seconds. This is a deliberate scope reduction, not an oversight: it covers
    /// every alignment reachable from real call sites.
    fn any_page_alignment() -> u64 {
        let choice: u8 = kani::any();
        kani::assume(choice < 3);
        match choice {
            0 => 4096,
            1 => 2 * 1024 * 1024,
            _ => 1024 * 1024 * 1024,
        }
    }

    /// A symbolic *well-formed* extent: exactly what `Extent::new_checked`
    /// accepts, i.e. non-empty and not wrapping past `u64::MAX`. Every extent-
    /// algebra proof below starts from two of these, so none of them can be
    /// vacuously discharged by feeding in a degenerate value the type is supposed
    /// to make unrepresentable.
    fn any_extent<T: AddressType>() -> Extent<T> {
        let address: u64 = kani::any();
        let length: u64 = kani::any();
        kani::assume(length != 0);
        kani::assume(length <= u64::MAX - address);
        Extent::new(Address::<T>::from_raw(address), Length::from_raw(length))
    }

    // ---------------------------------------------------------------------------
    // Alignment primitives
    //
    // `align_u64_down`/`align_u64_up` sit underneath every `Frame`/`Page`
    // constructor, `map_range`'s phase boundaries, and the bitmap allocator's
    // index math. Their doc comments promise "the largest value below x aligned to
    // alignment" / "the smallest value above" — these prove exactly that
    // three-part characterization (aligned, on the correct side, and *maximal* /
    // *minimal*), which is strictly stronger than checking the aligned bit alone.
    // ---------------------------------------------------------------------------

    #[kani::proof]
    fn align_down_is_greatest_aligned_at_or_below() {
        let x: u64 = kani::any();
        let alignment = any_alignment();

        let r = align_u64_down(x, alignment);

        assert!(r & (alignment - 1) == 0, "result is aligned");
        assert!(r <= x, "result does not exceed the input");
        // Maximality: nothing aligned sits strictly between `r` and `x`, i.e. the
        // gap is smaller than one alignment step.
        assert!(x - r < alignment, "result is the *greatest* such value");
    }

    #[kani::proof]
    fn align_up_is_least_aligned_at_or_above() {
        let x: u64 = kani::any();
        let alignment = any_alignment();
        // Precondition. `align_u64_up` computes `x + (alignment - 1)`, so it is
        // only meaningful when that sum fits; see `align_up_overflows_past_the_
        // last_aligned_value` for what happens when it doesn't.
        kani::assume(x <= u64::MAX - (alignment - 1));

        let r = align_u64_up(x, alignment);

        assert!(r & (alignment - 1) == 0, "result is aligned");
        assert!(r >= x, "result is not below the input");
        assert!(r - x < alignment, "result is the *least* such value");
    }

    /// The other half of `align_u64_up`'s (undocumented) contract: above the last
    /// aligned value it *always* overflows. `should_panic` makes that a proved
    /// statement rather than a comment — with overflow checks on this is a panic,
    /// and in a release build it would silently wrap to a small value, which is
    /// the genuinely dangerous outcome for a caller computing a mapping boundary.
    ///
    /// `Extent::expand_to_alignment` carries a `// TODO: handle if end_address
    /// extends beyond u64::MAX` acknowledging this same edge; this harness pins
    /// down precisely where the cliff is.
    #[kani::proof]
    #[kani::should_panic]
    fn align_up_overflows_past_the_last_aligned_value() {
        let x: u64 = kani::any();
        let alignment = any_alignment();
        kani::assume(alignment > 1);
        kani::assume(x > u64::MAX - (alignment - 1));

        let _ = align_u64_up(x, alignment);
    }

    #[kani::proof]
    fn address_alignment_helpers_agree_with_raw() {
        let raw: u64 = kani::any();
        let alignment = any_alignment();
        let a = PhysAddress::from_raw(raw);

        assert_eq!(a.align_down(alignment).as_raw(), align_u64_down(raw, alignment));
        // `is_aligned_to` is defined as a fixed point of `align_down`; prove that
        // matches the bit-level notion of alignment callers actually mean.
        assert_eq!(a.is_aligned_to(alignment), raw & (alignment - 1) == 0);
    }

    // ---------------------------------------------------------------------------
    // Extent construction invariants
    // ---------------------------------------------------------------------------

    /// `new_checked` is the gatekeeper for `Extent`'s two representation
    /// invariants: non-empty, and `address + length` does not wrap. Prove it
    /// accepts exactly the well-formed pairs — no false rejections either, since
    /// an over-strict check here would silently drop legitimate memory-map
    /// entries.
    #[kani::proof]
    fn new_checked_accepts_exactly_the_well_formed_extents() {
        let address: u64 = kani::any();
        let length: u64 = kani::any();

        let well_formed = length != 0 && length <= u64::MAX - address;
        let built = PhysExtent::new_checked(
            PhysAddress::from_raw(address),
            Length::from_raw(length),
        );

        assert_eq!(built.is_some(), well_formed);
        if let Some(e) = built {
            assert_eq!(e.address().as_raw(), address);
            assert_eq!(e.length().as_raw(), length);
            // The invariant callers depend on: the last address is representable,
            // so `last_address()` can't overflow downstream.
            assert!(address.checked_add(length - 1).is_some());
        }
    }

    /// FINDING (see `docs/kani-findings.md`, "empty extent"): `from_range_exclusive`
    /// bypasses `new_checked` and constructs the struct literally, so an empty
    /// range produces a zero-length `Extent` — a value the type's own constructor
    /// treats as unrepresentable. Proving it is reachable makes the gap explicit
    /// instead of latent.
    ///
    /// This is not a hypothetical: `PhysExtent::from_raw_range_exclusive(x, x)` is
    /// exactly what a UEFI memory-map entry describing an empty region would
    /// produce, and every downstream `last_address()` on such a value underflows.
    #[kani::proof]
    fn from_range_exclusive_can_violate_the_non_empty_invariant() {
        let x: u64 = kani::any();
        let e = PhysExtent::from_raw_range_exclusive(x, x);

        // The invariant `new_checked` enforces is *not* upheld here.
        assert_eq!(e.length().as_raw(), 0);
        assert!(
            PhysExtent::new_checked(e.address(), e.length()).is_none(),
            "the same (address, length) pair would be rejected by new_checked"
        );
    }

    /// Companion to the above: on a *well-formed* extent, `last_address` and
    /// `end_address` are total and consistent. This is what makes the zero-length
    /// case above a genuine hazard rather than a curiosity — every one of these
    /// accessors assumes the invariant.
    #[kani::proof]
    fn endpoints_are_consistent_on_well_formed_extents() {
        let e: PhysExtent = any_extent();

        let last = e.last_address();
        assert_eq!(last.as_raw(), e.address().as_raw() + e.length().as_raw() - 1);
        assert!(contains_addr(e, last), "last address is inside");
        assert!(contains_addr(e, e.address()), "start address is inside");

        // `end_address` is one past the last, and only exists when the extent
        // doesn't butt against the top of the address space.
        if e.address().as_raw() + e.length().as_raw() <= u64::MAX {
            assert!(!contains_addr(e, e.end_address()), "end address is outside");
        }
    }

    // ---------------------------------------------------------------------------
    // Extent algebra, proved against a membership specification
    //
    // `mark_kernel_areas` (memory.rs) composes overlap/left_difference/
    // right_difference to carve kernel-image ranges out of the UEFI memory map.
    // If any one of them is off by a byte, the frame allocator ends up handing out
    // a frame holding kernel code — so these are proved against an independent
    // set-membership spec, not against each other.
    // ---------------------------------------------------------------------------

    // Each of the four operations below is characterized by its two *endpoints*
    // rather than by a symbolic probe address. For intervals the two are
    // equivalent — two intervals are equal exactly when their endpoints agree —
    // but the endpoint form is both sharper (it names the answer instead of
    // asserting agreement with a predicate) and dramatically cheaper: a probe adds
    // a third fully symbolic 64-bit value on top of the two extents' four, and
    // CBMC does not settle those queries in useful time. The single-extent proofs
    // further down still use probes, where there is only one extent to quantify
    // over.
    //
    // The endpoint expressions here are written from the *definition* of each
    // operation, not read off the implementation.

    /// `overlap` is the set intersection: it starts at the later of the two
    /// starts, ends at the earlier of the two ends, and is `None` exactly when
    /// those cross.
    #[kani::proof]
    fn overlap_is_exactly_the_intersection() {
        let a: PhysExtent = any_extent();
        let b: PhysExtent = any_extent();

        let lo = max(a.address(), b.address());
        let hi = min(a.last_address(), b.last_address());

        match a.overlap(b) {
            Some(o) => {
                assert!(lo <= hi, "a non-empty result requires the ranges to meet");
                assert_eq!(o.address(), lo);
                assert_eq!(o.last_address(), hi);
            }
            None => assert!(lo > hi, "overlap() rejected a genuine intersection"),
        }
    }

    #[kani::proof]
    fn overlap_is_commutative() {
        let a: PhysExtent = any_extent();
        let b: PhysExtent = any_extent();

        assert_eq!(a.overlap(b), b.overlap(a));
    }

    #[kani::proof]
    fn has_overlap_matches_overlap() {
        let a: PhysExtent = any_extent();
        let b: PhysExtent = any_extent();

        assert_eq!(a.has_overlap(b), a.overlap(b).is_some());
    }

    /// `contains` is interval inclusion in both directions: it accepts exactly the
    /// pairs where `b`'s endpoints both lie within `a`. Getting this wrong in the
    /// permissive direction would let `mm::init`'s
    /// `assert!(init_alloc_extent.overlap(kernel_image).is_none())` pass while the
    /// bootstrap allocator is in fact carving frames out of the kernel image.
    #[kani::proof]
    fn contains_matches_interval_inclusion() {
        let a: PhysExtent = any_extent();
        let b: PhysExtent = any_extent();

        let included = a.address() <= b.address() && b.last_address() <= a.last_address();

        assert_eq!(a.contains(b), included);
    }

    /// `left_difference` is the part of `a` strictly below `b`: it keeps `a`'s
    /// start and stops just before `b` begins (or at `a`'s own end, if `a` lies
    /// entirely to the left).
    #[kani::proof]
    fn left_difference_is_the_part_strictly_left_of_other() {
        let a: PhysExtent = any_extent();
        let b: PhysExtent = any_extent();

        let nonempty = a.address() < b.address();

        match a.left_difference(b) {
            Some(d) => {
                assert!(nonempty);
                assert_eq!(d.address(), a.address());
                assert_eq!(
                    d.last_address(),
                    min(a.last_address(), b.address() - Length::from_raw(1))
                );
            }
            None => assert!(!nonempty),
        }
    }

    /// `right_difference` is the mirror image: the part of `a` strictly above `b`,
    /// starting just past `b`'s end (or at `a`'s own start) and keeping `a`'s end.
    ///
    /// The `Some` branch also proves the `unwrap` inside `right_difference` is
    /// total: it calls `other.end_address()`, a checked add, which the guard
    /// `self.last_address() <= other.last_address()` is what makes safe. That
    /// reasoning is asserted in a source comment; here it is checked.
    #[kani::proof]
    fn right_difference_is_the_part_strictly_right_of_other() {
        let a: PhysExtent = any_extent();
        let b: PhysExtent = any_extent();

        let nonempty = a.last_address() > b.last_address();

        match a.right_difference(b) {
            Some(d) => {
                assert!(nonempty);
                assert_eq!(
                    d.address(),
                    max(a.address(), b.last_address() + Length::from_raw(1))
                );
                assert_eq!(d.last_address(), a.last_address());
            }
            None => assert!(!nonempty),
        }
    }

    /// The composition `mark_kernel_areas` actually relies on: `a` is partitioned
    /// into (part left of `b`, part shared with `b`, part right of `b`) with no
    /// byte lost and none counted twice. A byte falling through this partition is
    /// a byte of physical memory that either vanishes from the memory map or gets
    /// handed out twice.
    ///
    /// Stated as lengths summing to `a`'s length, plus strict ordering between
    /// consecutive pieces. Together with the three endpoint characterizations
    /// above — which pin each piece exactly — that is a full partition proof, and
    /// it avoids the symbolic probe those harnesses used to carry.
    #[kani::proof]
    fn difference_and_overlap_partition_the_extent() {
        let a: PhysExtent = any_extent();
        let b: PhysExtent = any_extent();

        let left = a.left_difference(b);
        let mid = a.overlap(b);
        let right = a.right_difference(b);

        let len = |e: Option<PhysExtent>| e.map_or(0, |x| x.length().as_raw());

        assert_eq!(
            len(left) + len(mid) + len(right),
            a.length().as_raw(),
            "every byte of a is claimed exactly once"
        );

        // Contiguity and disjointness: consecutive pieces meet without gap or
        // overlap. `len` summing correctly plus this ordering is what makes the
        // three pieces a genuine partition rather than three ranges that merely
        // happen to add up.
        if let (Some(l), Some(m)) = (left, mid) {
            assert_eq!(m.address(), l.last_address() + Length::from_raw(1));
        }
        if let (Some(m), Some(r)) = (mid, right) {
            assert_eq!(r.address(), m.last_address() + Length::from_raw(1));
        }
        if let (Some(l), None, Some(r)) = (left, mid, right) {
            assert_eq!(r.address(), l.last_address() + Length::from_raw(1));
        }
    }

    #[kani::proof]
    fn join_contains_both_operands() {
        let a: PhysExtent = any_extent();
        let b: PhysExtent = any_extent();
        // `join` builds via `from_range_inclusive`, whose `+ 1` overflows when the
        // union reaches the very top of the address space. Exclude that; the
        // overflow itself is covered by `align_up_overflows_...`-style reasoning
        // and is not what this harness is about.
        let max_last = core::cmp::max(a.last_address(), b.last_address());
        let min_start = core::cmp::min(a.address(), b.address());
        kani::assume(max_last.as_raw() < u64::MAX);

        let j = a.join(b);

        // Minimality and coverage together: matching both endpoints exactly is
        // stronger than `j.contains(a) && j.contains(b)` and avoids leaning on
        // `overlap`.
        // Minimality: `join` is documented as "the smallest extent that contains
        // both", so it must not extend past either endpoint.
        assert_eq!(j.address(), min_start);
        assert_eq!(j.last_address(), max_last);
    }

    // ---------------------------------------------------------------------------
    // Alignment-adjusting extent operations
    //
    // `shrink_to_alignment` is what `iter_map_frames` uses to turn a byte-granular
    // UEFI memory-map entry into whole frames. If it ever returned a range
    // reaching outside the original entry, the frame allocator would mark a frame
    // free that isn't backed by usable RAM.
    // ---------------------------------------------------------------------------

    #[kani::proof]
    fn shrink_to_alignment_stays_inside_and_is_aligned() {
        let e: PhysExtent = any_extent();
        let alignment = any_page_alignment();
        // `any_extent` already guarantees the extent doesn't wrap, so
        // `end_address()`'s checked add is safe. No further precondition: this
        // harness used to need one because `shrink_to_alignment` rounded the start
        // up with an unchecked `address + (alignment - 1)` and panicked on an
        // extent starting in the last partial aligned block (see
        // `docs/kani-findings.md`, "shrink_to_alignment overflow"). It now returns
        // `None` there instead, so the function is total over every well-formed
        // extent.

        if let Some(s) = e.shrink_to_alignment(alignment) {
            assert!(s.address().is_aligned_to(alignment), "start is aligned");
            assert!(s.length().is_aligned_to(alignment), "length is aligned");
            // Raw endpoints rather than `e.contains(s)`, so this doesn't lean on
            // `overlap` (proved separately) and so the solver sees two integer
            // comparisons instead of `overlap`'s branch-and-recurse.
            assert!(s.address() >= e.address(), "never starts before the original");
            assert!(
                s.last_address() <= e.last_address(),
                "never ends after the original"
            );
        }
    }

    /// Maximality of `shrink_to_alignment`: the result is not merely *some*
    /// aligned sub-extent but the largest one, so no usable aligned frame is
    /// dropped from the memory map. Checked at a symbolic probe: any address that
    /// belongs to some aligned sub-extent of `e` belongs to the returned one.
    #[kani::proof]
    fn shrink_to_alignment_is_maximal() {
        let e: PhysExtent = any_extent();
        let alignment = any_page_alignment();
        // No precondition beyond well-formedness; see
        // `shrink_to_alignment_stays_inside_and_is_aligned`.

        let p: PhysAddress = PhysAddress::from_raw(kani::any());

        // The spec: `p` lies in `e` and is not in the unaligned head or tail.
        // `align_up_checked`, not `align_up` — an extent starting in the last
        // partial aligned block has no representable aligned start, and the spec
        // has to say so rather than panic computing itself.
        let Some(head) = e.address().align_up_checked(alignment) else {
            assert!(
                e.shrink_to_alignment(alignment).is_none(),
                "no aligned start is representable, so no aligned sub-extent exists"
            );
            return;
        };
        let tail = e.end_address().align_down(alignment).as_raw();
        let in_spec = p.as_raw() >= head.as_raw() && p.as_raw() < tail;

        match e.shrink_to_alignment(alignment) {
            Some(s) => assert_eq!(contains_addr(s, p), in_spec),
            None => assert!(!in_spec),
        }
    }

    #[kani::proof]
    fn expand_to_alignment_covers_the_original() {
        let e: PhysExtent = any_extent();
        let alignment = any_page_alignment();
        // Both the unstated preconditions of `expand_to_alignment`: the extent
        // must not wrap, and rounding its end up must not overflow. The latter is
        // the `// TODO: handle if end_address extends beyond u64::MAX` in the
        // source, stated executably.
        kani::assume(e.address().as_raw() <= u64::MAX - e.length().as_raw());
        kani::assume(e.end_address().as_raw() <= u64::MAX - (alignment - 1));

        let x = e.expand_to_alignment(alignment);

        assert!(x.address().is_aligned_to(alignment));
        assert!(x.length().is_aligned_to(alignment));
        // Raw endpoints, for the same reason as `shrink_to_alignment` above.
        assert!(x.address() <= e.address(), "covers the original's start");
        assert!(
            x.last_address() >= e.last_address(),
            "covers the original's end"
        );
    }

    // ---------------------------------------------------------------------------
    // Length arithmetic
    // ---------------------------------------------------------------------------

    /// `num_pages` is documented as "the minimum number of pages of length at
    /// least `self`". It computes `(n - 1 + PAGE_SIZE) / PAGE_SIZE`, which
    /// underflows at `n == 0` — so zero is an unstated precondition, proved here
    /// as the exact boundary.
    #[kani::proof]
    fn num_pages_is_the_ceiling_division() {
        let n: u64 = kani::any();
        kani::assume(n != 0);
        kani::assume(n <= u64::MAX - crate::memory::page::PAGE_SIZE.as_raw() + 1);

        let pages = Length::from_raw(n).num_pages();
        let page = crate::memory::page::PAGE_SIZE.as_raw();

        assert!(pages * page >= n, "enough pages to cover the length");
        assert!((pages - 1) * page < n, "and not one page more than needed");
    }

    #[kani::proof]
    #[kani::should_panic]
    fn num_pages_underflows_on_zero_length() {
        let _ = Length::from_raw(0).num_pages();
    }
}
