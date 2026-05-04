{
  description = "Development Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Derivação para o nuXmv
        nuxmv = pkgs.stdenv.mkDerivation rec {
          pname = "nuxmv";
          version = "2.1.0";

          src = pkgs.fetchurl {
            url = "https://nuxmv.fbk.eu/theme/download.php?file=nuXmv-${version}-linux64.tar.xz";
            sha256 = "sha256-x9/sQ3SbyyMMhX7+gQmbldhouU79n4G8zr5UKjBqfIM=";
          };

          nativeBuildInputs = with pkgs; [ autoPatchelfHook ];

          buildInputs = with pkgs; [
            stdenv.cc.cc.lib
            libxml2
            zlib
          ];

          installPhase = ''
            mkdir -p $out/bin
            cp bin/nuXmv $out/bin/
            chmod +x $out/bin/nuXmv
          '';
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-edit
            cargo-watch
            graphviz
            perf
            hyperfine
            pkgs.time
            nusmv
            nuxmv
            python3
          ];

          shellHook = ''
            echo "--- Model Checker Dev Environment ---"
            echo "nuXmv $(nuXmv --version | head -n 1) available."
            cargo --version
          '';
        };
      }
    );
}
