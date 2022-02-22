{
  inputs = {
    naersk.url = "github:nmattia/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, naersk, utils, flake-compat }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk {};
      in {
        # `nix build`
        defaultPackage = naersk-lib.buildPackage {
          pname = "nixos-systool";
          root = ./.;
        };
        # `nix run`
        defaultApp = utils.lib.mkApp {
            drv = self.defaultPackage."${system}";
        };
        # `nix develop`
        #
        # I'm using `rustup` here because Clion/Rust can't find the Rust stdlib
        # sources without it. I'm not sure what's going on there.
        devShell = with pkgs; mkShell {
          buildInputs = [ rustup rustfmt pre-commit rustPackages.clippy ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
        };
      });
}
