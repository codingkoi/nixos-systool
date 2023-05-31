{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    naersk = {
      url = "github:nmattia/naersk/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ self, nixpkgs, naersk, flake-parts, devshell, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } ({
      imports = [ devshell.flakeModule ];
      systems = [ "x86_64-linux" "x86_64-linux" "aarch64-darwin" ];
      perSystem = { pkgs, system, ... }:
        let
          inherit (nixpkgs) lib;
          inherit (lib.strings)
            concatMapStringsSep hasSuffix optionalString removePrefix;
          inherit (lib.lists) optional;

          naersk' = pkgs.callPackage naersk { };

          # MacOS specific stuff
          isDarwin = hasSuffix "-darwin" system;
          frameworks = pkgs.darwin.apple_sdk.frameworks;
          # Apple frameworks needed by the Notifications part of the tool
          darwinInputs = with frameworks; [
            Cocoa
            Foundation
            AppKit
            CoreServices
          ];
          # Generate Linker flags for Apple Frameworks from the list of Framework packages
          darwinLinkerFlags = concatMapStringsSep " " (lib:
            let libName = removePrefix "apple-framework-" lib.pname;
            in "-F${lib}/Library/Frameworks -framework ${libName}")
            darwinInputs;
          nativeBuildInputs = optional isDarwin darwinInputs;

          # package definition
          cargoToml = pkgs.lib.importTOML ./Cargo.toml;
          pname = cargoToml.package.name;
          package = naersk'.buildPackage {
            inherit pname;
            root = ./.;

            inherit nativeBuildInputs;
            NIX_LDFLAGS = optionalString isDarwin darwinLinkerFlags;
          };
        in {
          # `nix build`
          packages.default = package;
          # `nix run`
          apps.default.program = "${package}/bin/${pname}";
          # `nix develop`
          devshells.default = let
            ansiClear = "[H[2J";
            # Ansi 256 color code
            rustColor = "{166}";
          in {
            motd = ''
              ${ansiClear}${rustColor}{bold}🦀 Rust project - ${cargoToml.package.name} v${cargoToml.package.version}{reset}
              This is the devshell for developing on this project. Use whatever editor
              you're comfortable with to edit the code. The {166}{italic}rust-analyzer{reset} is
              available for use.

              Edit ${rustColor}{italic}flake.nix{reset} to change this greeting message.

              This code is licensed under ${cargoToml.package.license} using Rust ${cargoToml.package.edition} edition.
            '';
            packages = nativeBuildInputs ++ (with pkgs; [
              rustc
              cargo
              cargo-deny
              cargo-outdated
              cargo-readme
              clippy
              rustfmt
              rust-analyzer
            ]);
            env = [
              {
                name = "RUST_SRC_PATH";
                value = pkgs.rustPlatform.rustLibSrc;
              }
              {
                name = "NIX_LDFLAGS";
                value = optionalString isDarwin darwinLinkerFlags;
              }
            ];
          };
        };
    });
}
