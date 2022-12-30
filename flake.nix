{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        naersk' = pkgs.callPackage naersk {};

      in rec {
        defaultPackage = naersk'.buildPackage {
          src = ./.;
        };

        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            clippy
            rustfmt

	    pkg-config
	    openssl
          ];

          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;

          shellHook = ''
            alias e="cargo run --color=never"
            alias s="cargo clippy --color=never"
            alias f="cargo fmt"
          '';
        };
      }
    );
}
