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
        lists = pkgs.lib.lists;
        strings = pkgs.lib.strings;

        # MacOS specific stuff
        isDarwin = pkgs.stdenv.hostPlatform.isDarwin;
        frameworks = pkgs.darwin.apple_sdk.frameworks;
        # Inputs that are needed on any platform
        nativeInputs = with pkgs; [
          rust-analyzer
          rustc
          cargo
          cargo-outdated
          rustfmt
          clippy
        ];
        # Apple frameworks needed by the Notifications part of the tool
        darwinInputs = with frameworks; [
          Cocoa
          Foundation
          AppKit
          CoreServices
        ];
        # Generate Linker flags for Apple Frameworks from the list of Framework packages
        darwinLinkerFlags = strings.concatMapStringsSep " " (lib:
          let libName = strings.removePrefix "apple-framework-" lib.pname;
          in "-F${lib}/Library/Frameworks -framework ${libName}") darwinInputs;
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
            nativeBuildInputs = nativeInputs
              ++ lists.optional isDarwin darwinInputs;
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
            NIX_LDFLAGS = strings.optionalString isDarwin darwinLinkerFlags;
          };
      });
}
