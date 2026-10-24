{
  description = "bookshelfman development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system:
          f {
            inherit system;
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            };
          });
    in
    {
      devShells = forAllSystems ({ pkgs, ... }:
        let
          rust = pkgs.rust-bin.stable."1.99.1".default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
              "clippy"
              "rustfmt"
            ];
          };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rust
              cmake
              pkg-config
              perl
              stdenv.cc
            ];

            # btls-sys/BoringSSL builds cleanly with the stdenv GCC. Keep
            # clang out of PATH so CMake does not pick it automatically;
            # bindgen only needs libclang below.
            CC = "${pkgs.stdenv.cc}/bin/cc";
            CXX = "${pkgs.stdenv.cc}/bin/c++";

            # bindgen (used by btls-sys) needs libclang.
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

            # The test binaries link BoringSSL, which links libstdc++.
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
              pkgs.stdenv.cc.cc.lib
            ];

            RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library";

            shellHook = ''
              echo "bookshelfman development shell"
              echo "  rustc: $(rustc --version)"
              echo "  cargo: $(cargo --version)"
              echo "  cmake: $(cmake --version | head -n 1)"
              echo
              echo "  cargo build"
              echo "  cargo test --all-targets"
              echo "  cargo test --doc"
              echo "  cargo run"
            '';
          };
        });
    };
}
