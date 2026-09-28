# slotring

[![Crates.io](https://img.shields.io/crates/v/slotring.svg)](https://crates.io/crates/slotring)
[![Downloads](https://img.shields.io/crates/d/slotring.svg)](https://crates.io/crates/slotring)
[![Docs](https://docs.rs/slotring/badge.svg)](https://docs.rs/slotring)
[![CI](https://github.com/birchrust/slotring/actions/workflows/ci.yml/badge.svg)](https://github.com/birchrust/slotring/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/license/mit)

`slotring` keeps a fixed number of items in FIFO order. When it fills up, adding
a new item gives you back the oldest one. Its storage lives inside the ring, so
the capacity never grows.

Each item stays in the same slot for as long as it remains in the ring. You can
use that slot in an external index without tracking how the ring wraps around.
Once the item is evicted, its slot can be reused.

The crate works in `no_std` programs and has no dependencies. The ring itself
never allocates, although the values stored in it may.

## Usage

Add `slotring = "0.1"` to your Cargo dependencies. The minimum supported Rust
version is **1.85** (Rust 2024 edition).

Capacity is fixed by the nonzero const parameter `N`. Logical offsets follow
FIFO order, while physical slots remain stable for each live item and are
reused after eviction. A saved slot number is therefore not a permanent item
identity. A full ring replaces the oldest item on `push` and returns it to the
caller.

## License

Licensed under the MIT license. The license text is included in `LICENSE`.
