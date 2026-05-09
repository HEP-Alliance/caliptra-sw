# HEP Caliptra — ROM build and firmware signing

This fork of caliptra-sw targets a Caliptra implementation without the
Adams Bridge accelerator (no MLDSA87/MLKEM hardware). Firmware images are
signed with LMS (pure software SHA-256 verification, no hardware required).
The ROM variant is identified by the `no-adams-bridge-kat` feature flag.

---

## Prerequisites

Enter the Nix development shell. It provides Rust (via rustup), OpenSSL,
gcc, make, and everything else needed.

```sh
nix-shell        # from repo root
```

The first time, download the pinned Rust 1.85 toolchain and RISC-V target:

```sh
rustup show      # downloads toolchain if not already present
```

All commands below are run from `rom/dev/` inside `nix-shell` unless noted
otherwise.

```sh
cd rom/dev
```

---

## 1. Build the ROM

### ELF (for GDB/JTAG programming)

```sh
make build-no-adams-bridge
```

Output: `../../target/riscv32imc-unknown-none-elf/firmware/caliptra-rom`

This is a standard RISC-V ELF. Load it with GDB directly:

```gdb
target extended-remote :3333
load ../../target/riscv32imc-unknown-none-elf/firmware/caliptra-rom
monitor reset halt
continue
```

The ROM is linked at `0x00000000` (reset vector). Map your HyperRAM there
via the AXI address translator shim.

### Flat binary (for the emulator)

```sh
make build-rom-no-adams-bridge
```

Output: `../../target/riscv32imc-unknown-none-elf/firmware/caliptra-rom-no-adams-bridge.bin`

96 KB flat binary, identical content to the ELF, for use with `caliptra-emu`.

---

## 2. Generate signing keys

Keys live in the cargo target directory alongside the firmware artifacts.
Run this once to generate a fresh set of ECC-384 and LMS key pairs:

```sh
make gen-certs
```

This does three things:
- Calls into `caliptra-image-fake-keys` to generate LMS key pairs
  (`vnd-lms-{pub,priv}-key-{0..3}.pem`, `own-lms-{pub,priv}-key.pem`)
- Generates ECC-384 vendor and owner key pairs via OpenSSL
  (`vnd-{priv,pub}-key-{0..3}.pem`, `own-{priv,pub}-key.pem`)
- Copies pre-built MLDSA key binaries (not used for signing when
  `--pqc-key-type 3` is passed, but the image builder still expects them
  to be present)

All keys land in:
```
../../target/riscv32imc-unknown-none-elf/firmware/
```

**Keep the private key files. They are the only copy.** The LMS private key
contains the seed; losing it means you cannot sign further firmware images
with the same public key hash that you provision into fuses.

### Key structure

The image builder uses a `keys.toml` manifest (copied from
`tools/keys.toml`). It references:

- **Vendor keys** (slots 0–3): sign the FMC layer
- **Owner key** (one): signs the Runtime layer

For signing we use vendor slot 1 (`--ecc-pk-idx 1`) and LMS slot 0
(`--pqc-pk-idx 0`). Both can be changed on the image builder command line.

### Fuse provisioning

The ROM verifies firmware against the SHA-384 hash of the vendor public key
bundle, which must be burned into fuses at provisioning time. Compute it:

```sh
# After building a firmware image (step 3), the vendor PK hash is printed
# by the image builder as "Vendor PK hash: ...". Provision that value into
# the VENDOR_PK_HASH fuse bank.
```

In `unprovisioned` lifecycle mode (development) the ROM skips fuse
verification, so no fuse programming is needed during bring-up.

---

## 3. Build and sign a firmware image

Builds test FMC + test Runtime, signs with LMS (`--pqc-key-type 3`):

```sh
make build-fw-test-image PQC_KEY_TYPE=3
```

Output: `../../target/riscv32imc-unknown-none-elf/firmware/caliptra-rom-test-fw`

The image builder prints the vendor and owner public key hashes — save these
for fuse provisioning on real hardware.

To sign your own FMC and Runtime binaries instead of the test stubs,
call the image builder directly:

```sh
cargo run --manifest-path ../../image/app/Cargo.toml -- create \
    --pqc-key-type 3 \
    --key-config ../../target/riscv32imc-unknown-none-elf/firmware/keys.toml \
    --ecc-pk-idx 1 \
    --pqc-pk-idx 0 \
    --fmc    /path/to/your/fmc.elf \
    --fmc-version 1 \
    --fmc-rev $(git rev-parse HEAD) \
    --rt     /path/to/your/runtime.elf \
    --rt-version 1 \
    --rt-rev $(git rev-parse HEAD) \
    --fw-svn 0 \
    --out    /path/to/output/firmware.bin \
    --print-hashes
```

---

## 4. Run in the emulator (smoke test)

Builds ROM binary + LMS-signed test firmware and boots in `caliptra-emu`:

```sh
make run-no-adams-bridge DEVICE_LIFECYCLE=unprovisioned
```

Expected output includes:

```
[kat] LMS
[kat] MLDSA87 selftest SKIPPED (no-adams-bridge-kat)
[kat] --
[cold-reset] ++
...
[fwproc] Img verified w/ Vendor ECC Key Idx 1, PQC Key Type: LMS, PQC Key Idx 0, ...
[exit] Launching FMC @ 0x40000000
```

For production lifecycle (fuses provisioned) add `DEVICE_LIFECYCLE=production`.

---

## 5. What the feature flag does

`no-adams-bridge-kat` (in `kat/Cargo.toml` and `rom/dev/Cargo.toml`) gates
the MLDSA87 selftest in `kat/src/lib.rs`. The KAT data and driver code are
untouched — only the hardware self-exercise is skipped. MLKEM was already
excluded from ROM builds by an existing `#[cfg(not(feature = "rom"))]` guard.

LMS firmware verification (`drivers/src/lms.rs`) is pure software
(SHA-256 chain), so neither selftest nor firmware verification require the
Adams Bridge hardware block.

---

## Summary of artifacts

| File | Description |
|------|-------------|
| `target/.../caliptra-rom` | ROM ELF — load via GDB/JTAG |
| `target/.../caliptra-rom-no-adams-bridge.bin` | ROM flat binary — emulator |
| `target/.../firmware/vnd-lms-priv-key-0.pem` | LMS vendor signing key (keep safe) |
| `target/.../firmware/own-lms-priv-key.pem` | LMS owner signing key (keep safe) |
| `target/.../caliptra-rom-test-fw` | Signed firmware image |
