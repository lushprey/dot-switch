# ─────────────────────────────────────────────────────────────────────────────
# dot-switch — Nix flake
#
# Usage:
#   nix run github:lushprey/dot-switch                    # run without install
#   nix profile install github:lushprey/dot-switch        # install
#   nix develop                                           # drop into a dev shell
# ─────────────────────────────────────────────────────────────────────────────
{
  description = "dot-switch — swap ~/.config files between named profiles via symlinks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

        dotSwitch = pkgs.rustPlatform.buildRustPackage {
          pname = "dot-switch";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          # Runtime deps that the binary expects to call out to (optional):
          buildInputs = [ ];

          # Generate shell completions as part of the build
          postInstall = ''
            # Bash
            installShellCompletion --bash \
              --name dot-switch.bash \
              <($out/bin/dot-switch completions bash)
            # Zsh
            installShellCompletion --zsh \
              --name _dot-switch \
              <($out/bin/dot-switch completions zsh)
            # Fish
            installShellCompletion --fish \
              --name dot-switch.fish \
              <($out/bin/dot-switch completions fish)
          '';

          meta = with pkgs.lib; {
            description = "Swap ~/.config files between named profiles via symlinks";
            homepage = "https://github.com/lushprey/dot-switch";
            license = licenses.mit;
            maintainers = [ {
              name = "dot-switch contributors";
              email = "noreply@example.com";
              github = "lushprey";
            } ];
            mainProgram = "dot-switch";
            platforms = platforms.linux ++ platforms.darwin;
          };
        };
      in
      {
        packages = {
          default = dotSwitch;
          dot-switch = dotSwitch;
        };

        apps.default = {
          type = "app";
          program = "${dotSwitch}/bin/dot-switch";
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.cargo-dist
            pkgs.cargo-binstall
            pkgs.rust-analyzer
          ];
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
      });
}
