{
  description = "Nix flake template";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      treefmt-nix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        project = "Flake template";
      in
      {
        devShells.default = pkgs.mkShell {
          name = project;
          LSP_SERVER = "astro";
          packages = with pkgs; [
            # packages here
            bun
            astro-language-server
          ];
          shellHook = ''
            echo -e '(¬_¬") Entered ${project} :D'
          '';
        };
        formatter = treefmt-nix.lib.mkWrapper pkgs {
          projectRootFile = "flake.nix";
          programs = {
            # see list: https://github.com/numtide/treefmt-nix/tree/main/programs
            nixfmt.enable = true;
          };
        };
      }
    );
}
