use std::hint::black_box;
use std::time::Instant;

use slotring::Ring;

fn main() {
    const OPERATIONS: u64 = 1_000_000;
    let mut ring = Ring::<u64, 1024>::new();
    let mut checksum = 0_u64;

    let start = Instant::now();
    for value in 0..OPERATIONS {
        let (slot, evicted) = ring.push(black_box(value));
        checksum = checksum
            .wrapping_add(slot as u64)
            .wrapping_add(evicted.unwrap_or(0));
    }
    let elapsed = start.elapsed();

    black_box((ring, checksum));
    let operations = u128::from(OPERATIONS);
    let nanoseconds = elapsed.as_nanos();
    let whole = nanoseconds / operations;
    let hundredths = (nanoseconds % operations) * 100 / operations;
    println!("push: {OPERATIONS} operations in {elapsed:?} ({whole}.{hundredths:02} ns/op)");
}
