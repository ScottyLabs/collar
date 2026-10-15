{
  description = "ScottyLabs development CLI";

  nixConfig = {
    extra-substituters = [ "https://scottylabs.cachix.org" ];
    extra-trusted-public-keys = [
      "scottylabs.cachix.org-1:hajjEX5SLi/Y7yYloiXTt2IOr3towcTGRhMh1vu6Tjg="
    ];
  };

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    scottylabs = {
      url = "git+https://git.cmu.dev/ScottyLabs/kennel";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, scottylabs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          helpers = scottylabs.mkLib pkgs;
          docs = helpers.buildMdbook {
            src = ./.;
            name = "collar-docs";
          };
          collar = helpers.buildRustService {
            src = ./.;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildArgs.postInstall = ''
              wrapProgram $out/bin/collar --prefix PATH : ${
                pkgs.lib.makeBinPath [
                  pkgs.git
                  pkgs.git-credential-oauth
                ]
              }
            '';
          };
        in
        {
          inherit collar docs;
          default = collar;
        }
      );
    };
}
