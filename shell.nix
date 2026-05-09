{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "caliptra-rom-dev";

  nativeBuildInputs = with pkgs; [
    # Rust toolchain manager — reads rust-toolchain.toml (Rust 1.85,
    # riscv32imc-unknown-none-elf target).  Run `rustup show` once after
    # entering the shell to download the pinned toolchain if needed.
    rustup

    # C preprocessor used by build.rs to preprocess start.S
    gcc

    # OpenSSL is used by build.rs (fake-rom certificate generation) and
    # the openssl crate used in dev-dependencies.
    openssl
    pkg-config

    # Build tooling
    gnumake
    binutils

    # Handy for inspecting ELF firmware artifacts
    file
  ];

  # Tell openssl-sys (and any vendored openssl) to use the system library
  # rather than trying to compile it from source.
  OPENSSL_NO_VENDOR = "1";

  # pkg-config needs to find OpenSSL headers; mkShell sets PKG_CONFIG_PATH
  # automatically for nativeBuildInputs, but being explicit doesn't hurt.
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

  shellHook = ''
    echo "caliptra-rom dev shell"
    echo "  Rust toolchain : $(rustup show active-toolchain 2>/dev/null || echo 'not yet installed — run: rustup show')"
    echo "  Build targets  :"
    echo "    make build                  — standard ROM (EMU+CFI)"
    echo "    make build-no-adams-bridge  — ROM with MLDSA87/MLKEM selftests skipped"
  '';
}
