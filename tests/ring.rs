use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::string::String;
use std::vec::Vec;

use slotring::Ring;

#[test]
fn physical_slots_remain_stable_until_reused() {
    let mut items = Ring::<u64, 3>::new();
    assert_eq!(items.slot_at(0), None);
    assert_eq!(items.get_by_slot(0), None);
    for value in 10..13 {
        let _ = items.push(value);
    }
    let slot = items.slot_at(1).expect("second item is live");
    assert_eq!(items.get_by_slot(slot), Some(&11));
    let _ = items.push(13);
    assert_eq!(items.slot_at(0), Some(slot));
    assert_eq!(items.get_by_slot(slot), Some(&11));
    let (reused, evicted) = items.push(14);
    assert_eq!((reused, evicted), (slot, Some(11)));
    assert_eq!(items.get_by_slot(slot), Some(&14));
    assert_eq!(items.slot_at(3), None);
    assert_eq!(items.get_by_slot(3), None);
}

fn ring<T, const N: usize>() -> Ring<T, N> {
    Ring::new()
}

#[test]
fn reports_live_length() {
    let mut ring = ring::<_, 3>();

    assert_eq!(ring.len(), 0);

    assert_eq!(ring.push(1), (0, None));
    assert_eq!(ring.len(), 1);
}

#[test]
fn push_get_and_wraparound() {
    let mut ring = ring::<_, 3>();

    assert_eq!(ring.push(1), (0, None));
    assert_eq!(ring.push(2), (1, None));
    assert_eq!(ring.push(3), (2, None));
    assert_eq!(ring.get(0), Some(&1));
    assert_eq!(ring.get(2), Some(&3));

    assert_eq!(ring.push(4), (0, Some(1)));

    assert_eq!(ring.get(0), Some(&2));
    assert_eq!(ring.get(1), Some(&3));
    assert_eq!(ring.get(2), Some(&4));
    assert_eq!(ring.get(3), None);
}

#[test]
fn get_mut_updates_a_logical_slot_after_wraparound() {
    let mut ring = ring::<_, 3>();
    let _ = ring.push(1);
    let _ = ring.push(2);
    let _ = ring.push(3);
    assert_eq!(ring.push(4), (0, Some(1)));

    *ring.get_mut(1).expect("offset 1 must be present") = 30;

    assert_eq!(ring.as_slices(), (&[2, 30][..], &[4][..]));
    assert!(ring.get_mut(3).is_none());
}

#[test]
fn slices_are_empty_before_the_first_push() {
    let mut ring = ring::<_, 3>();
    assert_eq!(ring.as_slices(), (&[][..], &[][..]));

    let _ = ring.push(1);
    let _ = ring.push(2);
    assert_eq!(ring.as_slices(), (&[1, 2][..], &[][..]));
}

#[test]
fn capacity_one_overwrites() {
    let mut ring = ring::<_, 1>();

    assert_eq!(ring.push(1), (0, None));
    assert_eq!(ring.push(2), (0, Some(1)));
    assert_eq!(ring.get(0), Some(&2));
    assert_eq!(ring.as_slices(), (&[2][..], &[][..]));
}

#[test]
fn repeated_wraparound_preserves_logical_order() {
    let mut ring = ring::<_, 3>();

    for value in 0..100 {
        let _ = ring.push(value);
    }

    assert_eq!(ring.get(0), Some(&97));
    assert_eq!(ring.get(1), Some(&98));
    assert_eq!(ring.get(2), Some(&99));
    assert_eq!(ring.as_slices(), (&[97, 98][..], &[99][..]));
}

#[test]
fn range_iterates_across_the_wrap_boundary() {
    let mut ring = ring::<_, 5>();
    for value in 0..7 {
        let _ = ring.push(value);
    }

    assert_eq!(
        ring.range(1..5)
            .expect("valid logical range")
            .copied()
            .collect::<Vec<_>>(),
        [3, 4, 5, 6]
    );
    assert_eq!(ring.range(2..2).expect("valid empty range").next(), None);
    let reversed_start = 4;
    let reversed_end = 3;
    assert!(ring.range(reversed_start..reversed_end).is_none());
    assert!(ring.range(0..6).is_none());
}

#[test]
fn operations_match_vec_deque_model() {
    const CAPACITY: usize = 7;
    let mut ring = ring::<_, CAPACITY>();
    let mut model = VecDeque::with_capacity(CAPACITY);

    for value in 0..10_000_u32 {
        let expected = if model.len() == CAPACITY {
            model.pop_front()
        } else {
            None
        };
        model.push_back(value);
        assert_eq!(ring.push(value).1, expected);

        assert_eq!(ring.len(), model.len());
        let actual = (0..ring.len())
            .map(|offset| *ring.get(offset).expect("every live offset must be present"))
            .collect::<Vec<_>>();
        assert_eq!(actual, model.iter().copied().collect::<Vec<_>>());
        let (first, second) = ring.as_slices();
        assert_eq!(
            first.iter().chain(second).copied().collect::<Vec<_>>(),
            model.iter().copied().collect::<Vec<_>>()
        );
    }
}

#[test]
fn stores_non_copy_items() {
    let mut ring = ring::<_, 2>();

    assert_eq!(ring.push(String::from("a")), (0, None));
    assert_eq!(ring.push(String::from("b")), (1, None));
    assert_eq!(ring.push(String::from("c")), (0, Some(String::from("a"))));
    assert_eq!(ring.get(0).map(String::as_str), Some("b"));
    assert_eq!(ring.get(1).map(String::as_str), Some("c"));
}

#[test]
fn drops_evicted_and_live_items_once() {
    struct DropCounter(Rc<Cell<u32>>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let dropped = Rc::new(Cell::new(0));
    let mut ring = ring::<_, 2>();
    let _ = ring.push(DropCounter(Rc::clone(&dropped)));
    let _ = ring.push(DropCounter(Rc::clone(&dropped)));

    let (_, evicted) = ring.push(DropCounter(Rc::clone(&dropped)));
    assert_eq!(dropped.get(), 0);
    drop(evicted);
    assert_eq!(dropped.get(), 1);

    drop(ring);
    assert_eq!(dropped.get(), 3);
}
