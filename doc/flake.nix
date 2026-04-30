{
  description = "Tex env";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        
        tex = pkgs.texlive.combine {
          inherit (pkgs.texlive) 
            scheme-medium 
            abntex2 
            babel-portuguese 
            newtx 
            titlesec tocloft fancyhdr xcolor graphicx caption 
            hyperref lastpage indentfirst setspace geometry
            latexmk; 
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            tex
            pkgs.texstudio
          ];

          shellHook = ''
            echo "--- Tex Env ---"
            export TEXINPUTS=".:$TEXINPUTS"
          '';
        };
      });
}
