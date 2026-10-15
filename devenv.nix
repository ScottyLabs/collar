{ pkgs, inputs, ... }:
{
  imports = [ inputs.scottylabs.devenvModules.default ];

  scottylabs = {
    enable = true;
    project.name = "collar";
    rust.enable = true;
  };

  # Agent Skills require YAML frontmatter, which plain mdformat rewrites.
  treefmt.config.settings.formatter.mdformat.excludes = [ "**/SKILL.md" ];

  packages = with pkgs; [
    fd
    git
    git-credential-oauth
    jq
    mdbook
    openbao
    ripgrep
    secretspec
  ];

  scripts.docs.exec = ''exec mdbook serve "$DEVENV_ROOT" "$@"'';

  scripts.collar.exec = ''
    exec cargo run --quiet --manifest-path "$DEVENV_ROOT/Cargo.toml" -- "$@"
  '';
}
