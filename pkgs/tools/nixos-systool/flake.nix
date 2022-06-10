{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    utils.url = "github:numtide/flake-utils";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, utils, flake-compat }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.lib.${system};
        src = ./.;
        # Build *just* the cargo dependencies separately
        # so we don't rebuild them every time the code changes
        cargoArtifacts = craneLib.buildDepsOnly { inherit src; };

        crate = craneLib.buildPackage { inherit cargoArtifacts src; };
      in {
        # `nix build`
        packages.default = crate;
        # `nix run`
        defaultApp = utils.lib.mkApp { drv = self.defaultPackage."${system}"; };
        # `nix develop`
        #
        # I'm using `rustup` here because Clion/Rust can't find the Rust stdlib
        # sources without it. I'm not sure what's going on there.
        devShells.default = with pkgs;
          mkShell {
            nativeBuildInputs = [ rustup rust-analyzer rustfmt clippy ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
            shellHook = ''
              ${rustup}/bin/rustup update
            '';
          };
      });
}
