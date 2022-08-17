{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    naersk = {
      url = "github:nmattia/naersk/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    utils.url = "github:numtide/flake-utils";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, naersk, utils, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = naersk.lib."${system}";
      in rec {
        # `nix build`
        packages.default = naersk-lib.buildPackage {
          pname = "nixos-systool";
          root = ./.;
        };
        # `nix run`
        apps.default = utils.lib.mkApp { drv = packages.default; };
        # `nix develop`
        devShells.default = with pkgs;
          mkShell {
            nativeBuildInputs = [ rust-analyzer rustc cargo rustfmt clippy ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
          };
      });
}
