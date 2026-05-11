# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

See also [AGENTS.md](./AGENTS.md) for detailed crate layout, CI workflows, CLI usage, and publishing instructions.

## Workspace Structure

Three crates in a Cargo workspace (`resolver = "2"`, edition 2024):

- **`esp-nvs/`** — `no_std` embedded NVS library. Read/write/iterate key-value pairs on ESP32 NOR flash, format-compatible with ESP-IDF NVS.
- **`esp-nvs-partition-tool/`** — CLI + library to convert between NVS partition binaries and CSV.
- **`example/`** — ESP32C61 firmware demonstrating `esp-nvs` integration.

## Build & Test Commands

```bash
# Host tests (run on x86_64, no chip feature needed)
cargo test --workspace --all-targets
cargo test --workspace --doc

# Single crate
cargo test --package esp-nvs
cargo test --package esp-nvs-partition-tool

# Lint (nightly fmt check + clippy for both crates)
just lint

# Auto-fix clippy issues
just fix

# Nightly cargo fmt (edition 2024 requires nightly)
just fmt

# Check compilation for a specific chip target (no_std, no tests)
cargo check --package esp-nvs --features esp32c6 --target riscv32imac-unknown-none-elf
```

## Core Architecture (`esp-nvs/`)

The `Nvs<T: Platform>` struct is the central entry point. `Platform` is `Crc + NorFlash` (both from `embedded-storage`). Two blanket impls wire it up: `impl<T: Crc + NorFlash> Platform for T {}` and `impl<T: Platform> AlignedOps for T {}`.

**Initialization flow** (`Nvs::new` → `load_sectors` → `load_sector`):
1. Scan every 4k flash sector. Full pages are loaded into `self.pages` (a `Vec<ThinPage>`), empty pages into `self.free_pages` (a `BinaryHeap`).
2. Each page's header CRC is validated. Items are scanned and CRC-checked. Namespace definitions, blob indices, and blob data are tracked through the scan.
3. After scanning: `ensure_active_page_order` (active page must be last in vec), `continue_free_page` (resume interrupted defrag), `cleanup_duplicate_entries` (older duplicates erased), `cleanup_dirty_blobs` (orphaned chunk cleanup, version conflict resolution).

**Key types:**
- `Key` — 16-byte (15 + NUL), `BTreeMap`-compatible via `Ord`/`PartialOrd`
- `ThinPage` — in-memory representation of one flash page. Tracks state, sequence, entry bitmap, and a hash list for lookups. The actual `Item` data stays on flash and is loaded on demand.
- `u24` — custom 3-byte unsigned integer type for hash values
- `Item` — `#[repr(C, packed)]` 32-byte flash entry: namespace index, type, span, chunk index, CRC32, key, and a data union

**Item types on flash:** primitives (U8/I8/U16/I16/U32/I32/U64/I64), `Sized` (strings), `BlobIndex` + `BlobData` (multi-page blobs), `Blob` (legacy single-page).

**Blob versioning:** Blobs use a `VersionOffset` (V0=0x00, V1=0x80) scheme. When updating a blob, the new version writes to the opposite offset range, and the old version is deleted afterward. On init, if both versions exist, the older is removed.

**Page reclaim / wear leveling:** `defragment()` picks the page with the highest score (`erased_count * 10 + sequence_age`), copies live entries to a fresh page, then erases the source.

**Public API traits:** `Get<T>` and `Set<T>` provide a single overloaded `get`/`set` function. Implementations for `bool`, all integer widths, `String`, `Vec<u8>`. `Nvs::get` dispatches to `ItemType`-aware internal methods; `Nvs::set` writes-then-deletes the old entry to handle interruptions gracefully.

## `MemFlash` (host-side test harness)

In-memory `NorFlash` implementation at `esp-nvs/src/mem_flash.rs`. Simulates real NOR flash: erase sets `0xFF`, write is bitwise AND, 4-byte alignment, 4k sector erase. Uses `software_crc32()` for CRC. Used by all tests to avoid hardware dependencies.

## Public API

**Get/Set:** `Nvs::get::<T>(&mut self, namespace: &Key, key: &Key) -> Result<T, Error>` and `Nvs::set(&mut self, namespace: &Key, key: &Key, value: T) -> Result<(), Error>`. Supports `bool`, all integer widths (`u8`–`u64`, `i8`–`i64`), `String`, `Vec<u8>`.

**Iteration:** `nvs.keys()` returns `impl Iterator<Item = Result<(Key, Key), Error>>` (namespace, key pairs). `nvs.namespaces()` returns `impl Iterator<Item = &Key>`.

**Statistics:** `nvs.statistics() -> Result<NvsStatistics, Error>` returns per-page and overall entry counts (`empty`/`written`/`erased`) plus page state distribution (`empty`/`active`/`full`).

## Error Types (`esp-nvs/src/error.rs`)

`Error` is `#[non_exhaustive]`. Callers typically only need to handle `NamespaceNotFound` and `KeyNotFound`. Other variants: `InvalidPartitionOffset`, `InvalidPartitionSize`, `FlashError`, `NamespaceTooLong`, `NamespaceMalformed`, `ValueTooLong`, `KeyMalformed`, `KeyTooLong`, `ItemTypeMismatch(ItemType)`, `CorruptedData`, `FlashFull`, `PageFull` (internal).

## Logging

- **`defmt` feature** — enables `defmt::trace`/`warn` logging for embedded targets. Also derives `defmt::Format` on `Error`, `ItemType`, etc.
- **`debug-logs` feature** — enables `core::fmt`-based logging via `trace!`/`warn!` macros, for host-side debugging or when defmt isn't available.

## no_std Behavior

`#![cfg_attr(not(target_arch = "x86_64"), no_std)]` — the library is `no_std` on all embedded targets but uses `std` on x86_64 for host testing. `alloc` is always available (requires a global allocator on embedded). Tests always run on x86_64 with full std.

## Key Constraints

- Partition offset and size must both be multiples of 4096 (flash sector size).
- Max key length: 15 bytes (plus null terminator). Use `const { Key::from_str("k") }` to bake keys at compile time.
- Strings: max ~4000 bytes (fit on one page). Blobs: max ~500 kB (multi-page).
- Flash page layout is verified at compile time: `PAGE_HEADER_SIZE + 32 + 126 * 32 == 4096`.
- CRC32 uses IEEE 802.3 polynomial (0xEDB88320), zlib convention, compatible with `esp_hal::rom::crc::crc32_le`.
- On-chip, CRC delegates to ESP ROM. When a chip feature is enabled, `Crc` is auto-implemented for `FlashStorage` via `esp_hal::rom::crc::crc32_le`. On host (`MemFlash`), uses the software implementation.
- `edition = "2024"` everywhere — nightly required for `cargo fmt`.
