{
  description = "ScottyLabs development CLI";

  nixConfig = {
    extra-substituters = [ "https://scottylabs.cachix.org" ];
    extra-trusted-public-keys = [
      "scottylabs.cachix.org-1:hajjEX5SLi/Y7yYloiXTt2IOr3towcTGRhMh1vu6Tjg="
    ];
  };

  inputs = {
    scottylabs.url = "git+https://git.cmu.dev/ScottyLabs/kennel";
    nixpkgs.follows = "scottylabs/nixpkgs";
    flake-parts.follows = "scottylabs/flake-parts";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.scottylabs.flakeModules.default ];
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        {
          pkgs,
          lib,
          helpers,
          ...
        }:
        let
          docs = helpers.buildZensical {
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./zensical.toml
                ./docs
              ];
            };
            name = "collar-docs";
          };
          # Keep the skill in the source for src/skill.rs's include_str!
          collarSrc = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
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
                lib.makeBinPath [
                  pkgs.git
                  pkgs.git-credential-oauth
                ]
              }
            '';
          };
        in
        {
          scottylabs = {
            project.name = "collar";
            rust.enable = true;

            shell = {
              packages = with pkgs; [
                fd
                git
                git-credential-oauth
                jq
                zensical
                ripgrep
              ];

              scripts = {
                docs = ''cd "$SCOTTYLABS_ROOT" && exec zensical serve "$@"'';
                collar = ''exec cargo run --quiet --manifest-path "$SCOTTYLABS_ROOT/Cargo.toml" -- "$@"'';
              };
            };
          };

          # Agent Skills require YAML frontmatter, which plain mdformat rewrites
          treefmt.settings.formatter.mdformat.excludes = [ "**/SKILL.md" ];

          packages = {
            inherit collar docs;
            default = collar;
          };

          sites.docs = {
            package = docs;
            customDomain = "docs.collar.scottylabs.org";
          };

          checks = collar.passthru.checks;
        };
    };
}
