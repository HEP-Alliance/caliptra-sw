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

`no-adams-bridge-kat` now spans two crates and covers two distinct things,
for SoCs where Adams Bridge hardware is entirely absent (not just
defective):

- **`caliptra-kat/no-adams-bridge-kat`** gates the MLDSA87 selftest in
  `kat/src/lib.rs`. Only the boot-time self-exercise is skipped; the KAT
  data/logic is untouched. MLKEM was already excluded from ROM builds by an
  existing `#[cfg(not(feature = "rom"))]` guard.
- **`caliptra-common/no-adams-bridge-kat`** stubs `mldsa87_key_gen`,
  `mldsa87_sign`, `mldsa87_sign_and_verify`, and `mldsa87_verify` in
  `common/src/crypto.rs`. Without this, `IDevID`/`LDevID`/`FMC-alias`
  derivation (`rom/dev/src/flow/cold_reset/{idev_id,ldev_id,fmc_alias}.rs`)
  unconditionally calls real Adams Bridge hardware to generate/sign the
  MLDSA half of each layer's DICE identity keypair — with no Adams Bridge
  present, that hangs the bus rather than failing cleanly (there's no
  self-test-skip-equivalent for identity generation upstream; this fork adds
  one). With the stub: key generation returns an all-zero public key,
  signing returns an all-zero signature, and verification always succeeds.
  **This makes MLDSA identity/signature data cryptographically
  meaningless** — every device presents the same all-zero MLDSA pubkey, and
  any signature "verifies." Fine when MLDSA identity is being deliberately
  dropped (ECC alone still provides the real device-identity and
  measured-boot security property); dangerous if anything is later made to
  treat that data as meaningful.
  - Two hardware-touching call sites are *not* covered by this stub and
    remain real, since they're outside the normal cold-boot path:
    `FirmwareImageVerificationEnv::mldsa87_verify` (`common/src/verifier.rs`,
    only reached if a firmware image's manifest specifies MLDSA as its PQC
    key type — dormant as long as images are LMS-signed) and
    `MldsaVerifyCmd::execute` (`common/src/verify.rs`, the mailbox
    self-test-verify command — only reached if a mailbox client explicitly
    sends it).

LMS firmware verification (`drivers/src/lms.rs`) is pure software
(SHA-256 chain), so neither selftest nor firmware verification require the
Adams Bridge hardware block.

---

## 6. Unaligned scalar memory access (`.cargo/config.toml`)

`-C target-feature=+unaligned-scalar-mem` was removed from
`.cargo/config.toml`'s `riscv32imc-unknown-none-elf` rustflags. This is a
global change — it affects every build target in the repo, not just
`no-adams-bridge-kat` ones.

The flag tells LLVM the target CPU handles misaligned scalar loads/stores in
hardware. That's true for real Caliptra's VeeR EL2 core, but not verified
for the VexRiscv core this fork targets. With the flag enabled, LLVM
optimized some byte-array-to-word conversions — e.g.
`caliptra_drivers::array::u8_to_u32_be_impl`, used by
`Array4xN::from(&[u8; B])` — into a single unaligned `lw` whenever the
compiler happened to place the source array at a non-4-byte-aligned stack
address. On this platform that silently corrupted the loaded word (a
byte-duplication pattern) instead of trapping or working correctly.

This was the root cause of an HMAC-512Kdf KAT failure (investigated
2026-08-03/04): the KAT's key is a raw `[u8; 64]` needing this conversion;
HMAC-384Kdf's key is declared directly as a pre-converted word array and
never hit the bug, which is why only the 512-bit KAT failed. Confirmed via
waveform tracing that the corrupted data originated at the `lw` instruction
itself (CPU write to the HMAC key register already carried bad data), then
via disassembly comparison that removing the flag changes that function's
codegen to safe byte-wise `lbu` loads + shifts.

Removing the flag fixes this class of bug wherever it might occur (not just
the one call site found), at a small global code-size/perf cost. If
VexRiscv's LSU is later confirmed to handle unaligned scalar accesses
correctly (or is fixed to), the flag can be re-added.

---

## 7. DEVICE_LIFECYCLE — what it means and why Unprovisioned matters

The lifecycle state is a fuse-programmed value that the ROM reads at boot to
decide how strictly to enforce the security policy. It has nothing to do with
PUF enrollment or TRNG bring-up — it is purely a gate on fuse-based checks.

### Implications per state

| Check | Unprovisioned | Manufacturing | Production |
|---|---|---|---|
| Vendor public key hash vs fuse | **skipped** | enforced | enforced |
| PQC key type vs fuse | **skipped** | enforced | enforced |
| Anti-rollback / SVN | **skipped** | enforced | enforced |
| UDS (device secret) | all-zero | being written | burned |
| CDI (derived secret) | deterministic / same on all chips | meaningful | meaningful |
| UDS programming via mailbox | not allowed | **allowed** | not allowed |
| Debug unlock mechanism | open | simple token | signed challenge-response |
| `fake-rom` feature | allowed | allowed | rejected |

### The fuse bank stub insight

In Unprovisioned mode the ROM reads fuses, sees zeros, and **returns early
from every fuse-based check without error**. Zero is the explicit
"not yet provisioned" sentinel — it does not cause a fault, it causes a skip.

This has a very useful consequence for hardware bring-up: **your fuse bank
can be a stub that returns zero on all reads and the ROM will boot correctly**.
No fuse array, no PUF, no TRNG needed to get a working boot. A tie-to-zero on
the fuse interface is a valid integration strategy for a demonstrator.

Similarly the TRNG is used during IDevID key generation (the device identity
certificate path), but since IDevID/LDevID are PKI machinery that a
demonstrator does not need, a broken or absent TRNG only affects those paths.
The ROM will still boot, verify the LMS-signed firmware image, and hand off to
FMC.

The resulting hardware integration checklist for the HEP demonstrator:

| Block | Status | Impact |
|---|---|---|
| Fuse bank | stub / tie-to-zero | none — Unprovisioned skips all fuse checks |
| TRNG | open-source design / stub | none for boot; affects IDevID only |
| PUF | not needed | UDS is zero, CDI is deterministic, acceptable for demonstrator |
| Adams Bridge | absent from silicon | selftest skipped by `no-adams-bridge-kat` |

The security properties this removes are well-defined and intentional:
unique device identity, anti-rollback, and vendor key lock-in. The
**functional** behaviour — boot, firmware verification, FMC/Runtime execution
— is fully exercised. This is exactly the right scope for a tapeout
demonstrator aimed at validating the open-source silicon toolchain.

---

## Summary of artifacts

| File | Description |
|------|-------------|
| `target/.../caliptra-rom` | ROM ELF — load via GDB/JTAG |
| `target/.../caliptra-rom-no-adams-bridge.bin` | ROM flat binary — emulator |
| `target/.../firmware/vnd-lms-priv-key-0.pem` | LMS vendor signing key (keep safe) |
| `target/.../firmware/own-lms-priv-key.pem` | LMS owner signing key (keep safe) |
| `target/.../caliptra-rom-test-fw` | Signed firmware image |

---

## 6. LMS parameters (H5)

This fork signs with **LMS-SHA256-N24-H5 / LMOTS-SHA256-N24-W4** (tree type `10`,
OTS type `7`), not the upstream H15 tree. The constants live in
`image/types/src/lib.rs`:

```rust
pub const IMAGE_LMS_KEY_HEIGHT: usize = 5;
pub const IMAGE_LMS_TREE_TYPE: LmsAlgorithmType = LmsAlgorithmType::LmsSha256N24H5;
```

### Why

The image generator (`image/crypto/src/lib.rs`) only computes a real LM-OTS leaf
for trees of height <= 10 (`LMS_TREE_GEN_SUPPORTED_FULL_HEIGHT`). At H15 it
computes a real leaf **only at q = 5** (`SUPPORTED_LMS_Q_VALUE`) and zero-fills
the other 32767 leaves, so the H15 "tree" is mostly padding and exactly one leaf
is ever usable. At H5 the full 32-leaf tree is built, so every q is a genuine
one-time key.

The signer still hardcodes q = 5 (`OsslCrypto::lms_sign`); that was left alone,
it just now points at a real leaf.

### What this touches

The LMS algorithm itself (`drivers/src/lms.rs`) is **unchanged**. Only the
compile-time tree height changed, plus the three call sites that used the
non-generic `Lms::verify_lms_signature_cfi()`, which is hardcoded to
`LmsSignature<6, 51, 15>`. They now call `verify_lms_signature_cfi_generic()`:

- `common/src/verifier.rs` (ROM/FMC image verification)
- `runtime/src/set_auth_manifest.rs`
- `rom/dev/src/flow/fake.rs`

The LMS KAT (`kat/src/lms_kat.rs`) still uses its own H15 RFC 8554 test vector
through the non-generic entry point and is untouched.

### Manifest impact

**None on layout.** The LMS signature lives inside the fixed 4628-byte
`ImagePqcSignature` field, so the shorter H5 signature (1380 bytes instead of
1620 -- `tree_path` is 5 x 24 rather than 15 x 24) just leaves more zero padding.
Image and manifest offsets are unchanged.

**The ROM must be rebuilt and reflashed**, because it verifies against the
compile-time H. An H15-era ROM rejects an H5 signature with
`DRIVER_LMS_INVALID_TREE_HEIGHT`, and vice versa.

### Test key hashes

The baked-in LMS public keys in `image/fake-keys/src/lib.rs` were regenerated for
H5 (same `id` and `seed`, new `digest`), as were the documented hash values in
`rom/dev/README.md`. `image/fake-keys/tests/lms_keys_match.rs` re-derives each
public key from its private key and guards against them drifting apart again.
