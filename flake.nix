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

        nativeBuildInputs = lists.optional isDarwin darwinInputs;
      in rec {
        # `nix build`
        packages.default = naersk-lib.buildPackage {
          pname = "nixos-systool";
          root = ./.;

          inherit nativeBuildInputs;
          NIX_LDFLAGS = strings.optionalString isDarwin darwinLinkerFlags;
        };
        # `nix run`
        apps.default = utils.lib.mkApp { drv = packages.default; };
        # `nix develop`
        devShells.default = with pkgs;
          mkShell {
            nativeBuildInputs = nativeBuildInputs
              ++ [ rustc cargo cargo-outdated clippy rustfmt rust-analyzer ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
            NIX_LDFLAGS = strings.optionalString isDarwin darwinLinkerFlags;
          };
      });
}
