//! `slotring` provides a fixed-capacity FIFO ring for bounded history.
//!
//! [`Ring`] stores all slots inline. Pushing into a full ring replaces its
//! oldest item and returns both the reused physical slot and the evicted item.
//! Logical offsets follow FIFO order; each item keeps its physical slot until
//! eviction.

#![no_std]
#![allow(
    unsafe_code,
    reason = "the fixed-capacity storage invariant requires MaybeUninit and unchecked slot access"
)]
use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::ops::Range;
use core::{fmt, slice};

/// Fixed-capacity FIFO ring with inline storage.
///
/// Logical offset `0` refers to the oldest item. `N` is the number of physical
/// slots and must be greater than zero.
pub struct Ring<T, const N: usize> {
    items: [MaybeUninit<T>; N],
    head: usize,
    len: usize,
}

impl<T, const N: usize> Ring<T, N> {
    const CAPACITY_IS_NON_ZERO: () = assert!(N > 0, "Ring capacity must be greater than zero");

    /// Number of physical slots in the ring.
    pub const CAPACITY: usize = N;

    /// Creates an empty ring with `N` inline slots.
    ///
    /// `N` must be greater than zero; constructing a zero-capacity ring fails
    /// at compile time.
    #[must_use]
    #[allow(clippy::large_stack_frames)]
    pub const fn new() -> Self {
        let () = Self::CAPACITY_IS_NON_ZERO;

        Self {
            items: [const { MaybeUninit::uninit() }; N],
            head: 0,
            len: 0,
        }
    }

    /// Returns the number of live items.
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the ring contains no items.
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends `item` and returns `(slot, evicted)`.
    ///
    /// `evicted` is `Some` only when the ring was full. A live item keeps its
    /// physical slot until eviction, so external indexes can store the slot
    /// without tracking the moving head. A reused slot identifies the new item.
    #[must_use = "the returned tuple contains the slot and any evicted item"]
    #[inline]
    pub fn push(&mut self, item: T) -> (usize, Option<T>) {
        if self.len == Self::CAPACITY {
            let index = self.head;
            // SAFETY: `head` is always a valid index and a full ring has an
            // initialized item in every slot.
            let slot = unsafe { self.items.get_unchecked_mut(index) };
            // SAFETY: the full ring guarantees that `slot` is initialized.
            let evicted = unsafe { slot.assume_init_read() };
            slot.write(item);
            self.head = Self::next_index(index);
            return (index, Some(evicted));
        }

        // `head` remains zero until the first eviction, so the next free slot
        // is at `len`.
        let index = self.len;
        // SAFETY: `len < capacity`. The next slot is not initialized yet.
        unsafe { self.items.get_unchecked_mut(index) }.write(item);
        self.len += 1;
        (index, None)
    }

    /// Returns the physical slot at `offset` from the oldest item.
    ///
    /// Returns `None` if `offset >= len()`. The slot remains stable until this
    /// item is evicted.
    #[must_use]
    #[inline]
    pub fn slot_at(&self, offset: usize) -> Option<usize> {
        (offset < self.len).then(|| self.physical_index(offset))
    }

    /// Returns the item in a physical slot, or `None` if it is unfilled or
    /// outside the ring's capacity.
    ///
    /// Slots are reused after eviction. Callers retaining a slot should check
    /// the item's identity if it might have been replaced.
    #[must_use]
    #[inline]
    pub fn get_by_slot(&self, slot: usize) -> Option<&T> {
        if slot >= self.len {
            return None;
        }

        // SAFETY: initialized physical slots are exactly `0..len` before the
        // first eviction; once full, this covers every slot.
        let item = unsafe { self.items.get_unchecked(slot) };
        // SAFETY: the bounds check above establishes initialization.
        Some(unsafe { item.assume_init_ref() })
    }

    /// Returns a mutable reference to the item in a filled physical slot.
    ///
    /// Returns `None` for an unfilled or out-of-bounds slot. After eviction,
    /// the same slot refers to its replacement item. Unlike [`Self::get_mut`],
    /// `slot` is a physical index, not an offset from the oldest item.
    #[must_use]
    #[inline]
    pub fn get_by_slot_mut(&mut self, slot: usize) -> Option<&mut T> {
        if slot >= self.len {
            return None;
        }

        // SAFETY: before filling, initialized slots are exactly `0..len` and
        // head is zero. Once full, every slot is initialized. There is no pop
        // operation, so `slot < len` proves initialization in both states.
        let item = unsafe { self.items.get_unchecked_mut(slot) };
        // SAFETY: initialization is established above; the borrow is exclusive.
        Some(unsafe { item.assume_init_mut() })
    }

    /// Returns the item at `offset` from the oldest item, or `None` if the
    /// offset is outside the live range.
    #[must_use]
    #[inline]
    pub fn get(&self, offset: usize) -> Option<&T> {
        if offset >= self.len {
            return None;
        }

        // SAFETY: `offset < len` identifies an initialized live slot.
        Some(unsafe { self.get_unchecked(offset) })
    }

    /// Returns the mutable item at `offset` from the oldest item, or `None` if
    /// the offset is outside the live range.
    #[must_use]
    #[inline]
    pub fn get_mut(&mut self, offset: usize) -> Option<&mut T> {
        if offset >= self.len {
            return None;
        }

        let index = self.physical_index(offset);
        // SAFETY: `offset < len` identifies an initialized live slot and the
        // mutable Ring borrow guarantees exclusive access to it.
        let slot = unsafe { self.items.get_unchecked_mut(index) };
        // SAFETY: `offset < len` guarantees that `slot` is initialized.
        Some(unsafe { slot.assume_init_mut() })
    }

    /// Returns the live items as at most two slices in logical order.
    ///
    /// The first slice starts at the oldest item. The second slice is non-empty
    /// only when the live items wrap around the end of the inline storage. An
    /// empty ring returns two empty slices.
    #[must_use]
    #[inline]
    pub fn as_slices(&self) -> (&[T], &[T]) {
        let first_len = self.len.min(Self::CAPACITY - self.head);
        let second_len = self.len - first_len;
        let items = self.items.as_ptr().cast::<T>();

        // SAFETY: `N > 0` and `head < N`, so this points into the inline array.
        let first_ptr = unsafe { items.add(self.head) };
        // SAFETY: `head..head + first_len` contains initialized live items.
        let first = unsafe { slice::from_raw_parts(first_ptr, first_len) };
        // SAFETY: `0..second_len` contains initialized live items. `items` is
        // non-null and correctly aligned even when the slice is empty.
        let second = unsafe { slice::from_raw_parts(items, second_len) };
        (first, second)
    }

    /// Iterates over a logical half-open range from oldest to newest.
    ///
    /// Returns `None` when the range is reversed or extends past the live
    /// length. Empty ranges are valid. A wrapped range is exposed as one
    /// double-ended iterator without allocation.
    #[must_use]
    #[inline]
    pub fn range(&self, range: Range<usize>) -> Option<impl DoubleEndedIterator<Item = &T> + '_> {
        if !(range.start..=self.len).contains(&range.end) {
            return None;
        }

        let (first, second) = self.as_slices();
        let boundary = first.len();
        let (range_first, range_second) = if range.start < boundary {
            let first_end = range.end.min(boundary);
            let second_end = range.end.saturating_sub(boundary);
            (&first[range.start..first_end], &second[..second_end])
        } else {
            let start = range.start - boundary;
            let end = range.end - boundary;
            (&second[start..end], &second[0..0])
        };

        Some(range_first.iter().chain(range_second.iter()))
    }

    /// Binary searches items sorted in logical order using `compare`.
    ///
    /// Returns a logical offset on a match, or the logical insertion offset on
    /// a miss, just like [`slice::binary_search_by`]. With duplicates, any
    /// matching offset may be returned. `compare` must give a monotonic ordering
    /// across items in logical order; otherwise the result is unspecified.
    ///
    /// The search selects a contiguous slice before comparing its items, so it
    /// does not need to convert each logical offset to a physical index.
    ///
    /// # Errors
    ///
    /// Returns `Err(offset)` when no item matches. `offset` is the logical
    /// position where the searched value could be inserted.
    #[inline]
    pub fn binary_search_by<F>(&self, mut compare: F) -> Result<usize, usize>
    where
        F: FnMut(&T) -> Ordering,
    {
        let (first, second) = self.as_slices();
        if let Some(pivot) = second.first() {
            match compare(pivot) {
                Ordering::Equal => return Ok(first.len()),
                Ordering::Less => {
                    return second
                        .binary_search_by(compare)
                        .map(|index| first.len() + index)
                        .map_err(|index| first.len() + index);
                }
                Ordering::Greater => {}
            }
        }
        first.binary_search_by(compare)
    }

    /// # Safety
    ///
    /// `offset` must identify a live item (`offset < self.len`).
    #[inline]
    unsafe fn get_unchecked(&self, offset: usize) -> &T {
        let index = self.physical_index(offset);
        // SAFETY: the caller guarantees `offset < len`, so the mapped physical
        // slot is in bounds and initialized.
        let slot = unsafe { self.items.get_unchecked(index) };
        // SAFETY: the caller guarantees that the mapped slot is initialized.
        unsafe { slot.assume_init_ref() }
    }

    // Maps an in-bounds logical offset to a physical index.
    #[inline]
    fn physical_index(&self, offset: usize) -> usize {
        debug_assert!(offset < Self::CAPACITY, "logical offset must be in bounds");
        let distance_to_end = Self::CAPACITY - self.head;
        if offset < distance_to_end {
            self.head + offset
        } else {
            offset - distance_to_end
        }
    }

    // Advances a physical index, wrapping from the last slot to zero.
    #[inline]
    fn next_index(index: usize) -> usize {
        debug_assert!(index < Self::CAPACITY, "physical index must be in bounds");
        let next = index + 1;
        if next == Self::CAPACITY { 0 } else { next }
    }

    /// Returns the newest item, or `None` if the ring is empty.
    #[must_use]
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.len.checked_sub(1).map(|offset| {
            // SAFETY: `len - 1` is a valid live logical offset.
            unsafe { self.get_unchecked(offset) }
        })
    }

    /// Returns the mutable newest item, or `None` if the ring is empty.
    #[must_use]
    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut T> {
        self.len.checked_sub(1).map(|offset| {
            let index = self.physical_index(offset);

            // SAFETY: `offset == len - 1` identifies an initialized live slot.
            let slot = unsafe { self.items.get_unchecked_mut(index) };

            // SAFETY: the slot is initialized and `&mut self` grants exclusive access.
            unsafe { slot.assume_init_mut() }
        })
    }

    /// Iterates over live items from oldest to newest.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let (first, second) = self.as_slices();
        first.iter().chain(second.iter())
    }
}

impl<T, const N: usize> Default for Ring<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for Ring<T, N> {
    fn drop(&mut self) {
        for offset in 0..self.len {
            let index = self.physical_index(offset);
            // SAFETY: every logical offset in `0..len` maps to one distinct,
            // initialized live slot.
            let slot = unsafe { self.items.get_unchecked_mut(index) };
            // SAFETY: every live slot is initialized and visited exactly once.
            unsafe { slot.assume_init_drop() };
        }
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for Ring<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ring")
            .field("items", &DebugItems(self))
            .field("capacity", &N)
            .finish()
    }
}

struct DebugItems<'a, T, const N: usize>(&'a Ring<T, N>);

impl<T: fmt::Debug, const N: usize> fmt::Debug for DebugItems<'_, T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for offset in 0..self.0.len {
            // SAFETY: every offset in `0..len` identifies an initialized slot.
            list.entry(unsafe { self.0.get_unchecked(offset) });
        }
        list.finish()
    }
}
