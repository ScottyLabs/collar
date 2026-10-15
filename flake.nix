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
          # Keep the skill in the source for src/skill.rs's include_str!
          collarSrc = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
              ./docs/skills/collar/SKILL.md
            ];
          };
          collar = helpers.buildRustService {
            src = ./.;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildArgs.src = collarSrc;
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
