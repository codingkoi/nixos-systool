# NixOS system management tool

This is a helper tool I wrote to manage my NixOS systems. It has a decent set of features and works for me, but it might not work for you.

I mostly wrote it so I wouldn't have to remember the lower level Nix commands (and if I forgot, the commands are in the source code). 

```
NixOS system management tool

Usage: nixos-systool [OPTIONS] --flake-path <FLAKE_PATH> <COMMAND>

Commands:
  apply         Apply the system configuration using nixos-rebuild
  apply-user    Apply user configuration using home-manager
  clean         Run garbage collection on the Nix store
  build         Build the system configuration, without applying it
  prune         Prune old generations from the Nix store
  search        Search Nixpkgs or NixOS options
  update        Update the system flake lock
  check         Check if the flake lock is outdated
  print-config  Print the currently loaded configuration including defaults
  help          Print this message or the help of the given subcommand(s)

Options:
  -f, --flake-path <FLAKE_PATH>
          Path to the system configuration flake repository [env: SYS_FLAKE_PATH=/home/jeremy/.config/nixos]
  -c, --current-flake-path <CURRENT_FLAKE_PATH>
          Path to the current system flake in the Nix store [default: /etc/current-system-flake]
  -h, --help
          Print help information
  -V, --version
          Print version information
```

## TODO - document this more
