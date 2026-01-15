{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-25.11";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay.url = "github:oxalica/rust-overlay";
    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } ({
      imports = [ inputs.devshell.flakeModule ];
      systems = [ "x86_64-linux" "x86_64-darwin" "aarch64-darwin" ];
      perSystem = { config, self', pkgs, lib, system, ... }:
        let
          runtimeDeps = with pkgs; [ ];
          buildDeps = with pkgs; [ pkg-config rustPlatform.bindgenHook ];
          devDeps = with pkgs;
            [
              #gdb # Not available on Darwin
            ];

          cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
          msrv = cargoToml.package.rust-version;

          rustPackage = features:
            (pkgs.makeRustPlatform {
              cargo = pkgs.rust-bin.stable.latest.minimal;
              rustc = pkgs.rust-bin.stable.latest.minimal;
            }).buildRustPackage {
              inherit (cargoToml.package) name version;
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
              buildFeatures = features;
              buildInputs = runtimeDeps;
              nativeBuildInputs = buildDeps;
            };

        in {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };

          # `nix build`
          packages.default = self'.packages.nixos-systool;
          packages.nixos-systool = rustPackage "";
          # `nix run`
          apps.default.program =
            "${self'.packages.nixos-systool}/bin/${cargoToml.package.name}";
          # `nix develop`
          devshells.default = let
            # Ansi 256 color code for rust orange
            rustColor = "{166}";
          in {
            motd = ''

              ${rustColor}{bold}🦀 Rust project - ${cargoToml.package.name} v${cargoToml.package.version}{reset}
              This is the devshell for developing on this project. Use whatever editor
              you're comfortable with to edit the code. The ${rustColor}{italic}rust-analyzer{reset} is
              available for use.

              Edit ${rustColor}{italic}flake.nix{reset} to change this greeting message.

              This code is licensed under ${cargoToml.package.license} using Rust ${cargoToml.package.edition} edition.
            '';
            packages = with pkgs; [
              rust-bin.stable.latest.default
              rust-analyzer
              cargo-deny
              cargo-outdated
              cargo-readme
            ];
            # env = [
            #   {
            #     name = "RUSTC_VERSION";
            #     value = toolchainToml.toolchain.channel;
            #   }
            #   {
            #     name = "RUSTFLAGS";
            #     value = concatStringsSep " " (map (a: "-L ${a}/lib") [ ]);
            #   }
            #   # Add glibc, clang, glib and other headers to bindgen search path
            #   {
            #     name = "BINDGEN_EXTRA_CLANG_ARGS";
            #     # Includes with normal include path
            #     value = concatStringsSep " "
            #       # Includes with special directory paths
            #       [
            #         ''
            #           -I"${pkgs.llvmPackages_latest.libclang.lib}/lib/clang/${pkgs.llvmPackages_latest.libclang.version}/include"''
            #         ''-I"${pkgs.glib.dev}/include/glib-2.0"''
            #         "-I${pkgs.glib.out}/lib/glib-2.0/include/"
            #       ];
            #   }
            #   {
            #     name = "LIBCLANG_PATH";
            #     value =
            #       makeLibraryPath [ pkgs.llvmPackages_latest.libclang.lib ];
            #   }
            #   {
            #     name = "RUST_SRC_PATH";
            #     value = pkgs.rustPlatform.rustLibSrc;
            #   }
            #   {
            #     name = "PATH";
            #     eval =
            #       "$PATH:\${CARGO_HOME:~/.cargo}/bin:\${RUSTUP_HOME}:~/.rustup}/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/";
            #   }
            # ];
          };
        };
    });
}
