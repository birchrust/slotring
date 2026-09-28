use slotring::Ring;

fn main() {
    let mut ring = Ring::<&str, 3>::new();

    for item in ["first", "second", "third", "fourth"] {
        let (slot, evicted) = ring.push(item);
        println!("slot {slot}: inserted {item}, evicted {evicted:?}");
    }

    let items: Vec<_> = ring.iter().copied().collect();
    println!("oldest to newest: {items:?}");

    let slot = ring.slot_at(0).expect("the ring is not empty");
    println!(
        "oldest item is in slot {slot}: {:?}",
        ring.get_by_slot(slot)
    );
}
