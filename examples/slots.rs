use slotring::Ring;

fn main() {
    let mut ring = Ring::<&str, 2>::new();
    let (slot, evicted) = ring.push("first");
    assert_eq!(evicted, None);

    let _ = ring.push("second");
    assert_eq!(ring.get_by_slot(slot), Some(&"first"));

    let (reused, evicted) = ring.push("third");
    assert_eq!((reused, evicted), (slot, Some("first")));
    assert_eq!(ring.get_by_slot(slot), Some(&"third"));

    println!("slot {slot} now contains the replacement item");
}
