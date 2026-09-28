use slotring::Ring;

fn main() {
    let mut ring = Ring::<i32, 3>::new();
    for value in [1, 3, 5, 7] {
        let _ = ring.push(value);
    }

    // After the eviction, the logical order is [3, 5, 7].
    assert_eq!(ring.binary_search_by(|item| item.cmp(&5)), Ok(1));
    assert_eq!(ring.binary_search_by(|item| item.cmp(&6)), Err(2));

    println!("sorted live items: {:?}", ring.iter().collect::<Vec<_>>());
}
