# esp-nvs — Agent Guide

## Project Overview

Rust workspace: ESP-IDF compatible NVS (Non-Volatile Storage) library + partition tool.

- **`esp-nvs/`** — no-std library for reading/writing NVS on ESP32 chips (bare metal)
- **`esp-nvs-partition-tool/`** — CLI + library to parse/generate NVS partition binaries from/to CSV
- **`example/`** — ESP32C61 binary with `esp-nvs` integration demo

## 5‑Second Setup

| Command | What it does |
|---|---|
| `cargo test --workspace --all-targets` | Run all host tests |
| `cargo test --workspace --doc` | Run doc tests |
| `cargo test --package esp-nvs` | NVS library tests only |
| `cargo test --package esp-nvs-partition-tool` | Partition tool tests only |
| `just` | List available `just` recipes |
| `just lint` | Clippy with `-D warnings` on both crates |
| `just fix` | Auto-fix clippy issues |
| `just test` | `cargo test --all && cargo test --doc` |

## Critical Conventions

### Chip Features (always pick exactly one)

`esp-nvs` requires a chip feature. Supported: `esp32`, `esp32s2`, `esp32s3`, `esp32c2`, `esp32c3`, `esp32c5`, `esp32c6`, `esp32c61`, `esp32h2`.

Feature gates: `esp-storage`, `esp-hal`, `esp-sync`, `esp-rom-sys` (all optional with chip features).
Target arch: `riscv32` chips use `riscv` crate; `xtensa` chips use `xtensa-lx` crate.
Example: `cargo test --features esp32c6` fails because tests run on x86_64. Host tests use `MemFlash` with no chip feature.

### Formatting

- **Edition 2024** (all crates). `rustfmt.toml`: max_width 120, `StdExternalCrate` grouping, `Module` granularity.
- **Nightly** required for `cargo fmt`. Use:
  ```
  just fmt                          # nightly fmt
  just _nightly-fmt-check           # nightly fmt --check
  cargo fmt --all                   # ⚠ may fail on nightly-only config
  ```

### Code Layout (esp-nvs)

| File | Role |
|---|---|
| `src/lib.rs` | `Nvs` struct, `Key` type (15+1 bytes), public re-exports |
| `src/platform.rs` | `Platform` trait (Crc + NorFlash), `software_crc32()`, `AlignedOps` |
| `src/internal.rs` | Page management, namespace caching, init/read/write logic |
| `src/raw.rs` | Flash page layout (const assertions), `Item`, `PageHeader`, entry state bitmap |
| `src/error.rs` | `Error` enum (non_exhaustive) |
| `src/get.rs` / `src/set.rs` | `Get<T>` / `Set<T>` trait impls for primitives, String, Vec<u8> |
| `src/mem_flash.rs` | `MemFlash` — host-side in-memory NorFlash (for tests) |
| `src/u24.rs` | 3-byte unsigned int type |

### Key API Facts

- **Key length**: Max 15 bytes + 1 null terminator = 16 bytes total. Use `Key::from_str("key")` or `Key::from_array(b"key")`. Prefer `const` context: `let K = const { Key::from_str("my_key") };`.
- **Nvs::new(offset, size, flash)** — offset must be sector-aligned (4k), size must be multiple of 4k.
- **Get/Set** generic: supports `bool, u8, i8, u16, i16, u32, i32, u64, i64, String, Vec<u8>`
- **Flash page layout**: compile-time assertion validates `PAGE_HEADER_SIZE + 32 + 126*32 == 4096`.
- **CRC32**: Uses IEEE 802.3 polynomial (`0xEDB88320`), zlib convention. Compatible with `libz_sys::crc32` and ESP-IDF ROM `crc32_le`. Built-in `software_crc32()` for host platforms.
- **EntryStatistics / NvsStatistics / PageStatistics** available for flash usage introspection.

### Code Layout (esp-nvs-partition-tool)

| File | Role |
|---|---|
| `src/lib.rs` | `NvsPartition` struct, `try_from()`, `to_csv()`, `generate_partition()` |
| `src/partition/` | Binary parser (`parser`) and generator (`generator`) |
| `src/csv/` | CSV parser (`parser`), writer (`writer`), row types |
| `src/bin/main.rs` | CLI with `generate` and `parse` subcommands (clap) |
| `src/error.rs` | Error enum (wraps CSV/IO/hex/base64/NVS errors) |

**Auto-detect format**: `NvsPartition::try_from()` checks if first byte ≥ 0x80 (binary NVS) or < 0x80 (CSV text).

### Partition Tool CLI

```
esp-nvs-partition-tool generate <input.csv> <output.bin> --size <bytes>   # 0x prefix for hex
esp-nvs-partition-tool parse <input.bin> <output.csv>
```

### Partition Tool CSV Format

Four columns: `key,type,encoding,value`
- `type` = `namespace`, `data`, or `file`
- `encoding` = `u8|i8|u16|i16|u32|i32|u64|i64|string|hex2bin|base64|binary`
- File paths in `file` entries are resolved relative to the CSV file's directory.

### test_nvs_data.bin

Generated from `test_nvs_data.csv` via `esp-nvs-partition-tool generate`. To regenerate:
```bash
esp-nvs-partition-tool generate esp-nvs/tests/assets/test_nvs_data.csv esp-nvs/tests/assets/test_nvs_data.bin --size 0x4000
```

### Partition Tool Test Assets

Located at `esp-nvs-partition-tool/tests/assets/`. Roundtrip tests parse CSV → binary → CSV → binary → parse, verifying data integrity at each step. All tests run on `x86_64` host with no hardware.

## Infrastructure

### Dev Environment (Nix + devenv + direnv)

- `.envrc` loads `devenv` (direnv-based)
- `devenv.nix` provides: Rust `1.93.1` stable, `just`, `nixfmt`, `cargo-edit`, `actionlint`, `esp-nvs-partition-tool` via overlay
- Targets installed: `x86_64-unknown-linux-gnu`, `riscv32imac-unknown-none-elf`, `riscv32imc-unknown-none-elf`
- Cross-target for xtensa uses separate `xtensa-toolchain` action in CI

### CI (GitHub Actions)

| Workflow | Trigger | Key commands |
|---|---|---|
| `check.yml` | push/pr | `cargo fmt --all --check` (nightly), clippy stable+beta (with chip targets), `cargo check --workspace --all-targets`, doc (nightly) |
| `nostd.yml` | push/pr | `cargo check` for riscv32imc, riscv32imac, xtensa-esp32 (all no-default-features) |
| `test.yml` | push/pr | `cargo test --locked --workspace --all-targets` + `--doc` on stable+beta + macos |

CI runs clippy per-package with `--features=defmt` for `esp-nvs` (to catch defmt-related issues).

### Workspace Profile

```toml
[profile.dev.package.esp-storage]
opt-level = 3
```

The example crate uses `opt-level = "s"` even in dev profile (constrained device).

### Commit Convention

Conventional commits (enforced by `cliff.toml` + git-cliff). Changelogs auto-generated per crate:
```bash
just nvs::update-changelog       # esp-nvs changelog
just partition_tool::update-changelog   # partition tool changelog
```

### Publishing

```bash
just nvs::publish-dry-run
just nvs::publish
just partition_tool::publish-dry-run
just partition_tool::publish
```

### Lint & Fix

```bash
just lint                                 # Both crates with -D warnings
just fix                                  # Auto-fix + cargo fmt
cargo clippy --release -p esp-nvs --features=defmt -- -D warnings
cargo clippy --release -p esp-nvs-partition-tool -- -D warnings
```

## Gotchas

- **`edition = "2024"`** — ensure nightly toolchain for `cargo fmt` (edition 2024 formatting rules are nightly-only).
- **no-std** is active on all non-x86_64 targets. `alloc` is still available but `std` is not. Tests run on x86_64 with std.
- **Flash semantics**: `MemFlash` simulates real NOR flash — erase sets `0xFF`, write is bitwise AND. Alignment: 4-byte reads/writes, 4096-byte erases.
- **`unsafe`** blocks exist for `#[repr(C,packed)]` struct transmutes and union access. All protected by CRC32 checksums.
- **defmt logging**: Enable `--features=defmt` for defmt trace/warn in the NVS driver. Also has `debug-logs` feature for `core::fmt`-based logging.
- **Key comparison**: `Key` derives `Ord`, `PartialOrd`, making it usable as `BTreeMap` keys (used internally for blob index).
- **The partition tool binary** depends on the `cli` feature (default-on). Disable with `--no-default-features` for lib-only usage.
- **Example** targets `esp32c61` only. Adjust `partition_offset`/`partition_size` for different partition tables.
