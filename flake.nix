#
# Build environment for USRs.
#
# vim: et:ts=2:sw=2:
#
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-22.11";

    # Convenience helpers for making flakes tidier.
    flake-utils.url = "github:numtide/flake-utils";

    # The Rust overlay utilities allow us to pin ourselves deterministically
    # to a Rust version other than the one Nix is using; so we can interop more easily.
    rust-overlay.url = "github:oxalica/rust-overlay";

  };

  description = "Universal Serial Rust library & tools";
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:

    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
      in
      rec {

        devShell = pkgs.mkShell {
          buildInputs = buildDepends ++ (with pkgs; [
            rustc
            cargo
            rust-analyzer
            rustfmt
          ]);
        };

        #
        # Core library.
        #
        defaultPackage = pkgs.rustPlatform.buildRustPackage {
          name = "usrs";
          src = ./.;

          buildInputs = buildDepends;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          meta = {
            description = "Rust-native USB host library for Rust";
          };

        };

      });
}
