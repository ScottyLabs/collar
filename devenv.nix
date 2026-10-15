{ pkgs, inputs, ... }:
{
  imports = [ inputs.scottylabs.devenvModules.default ];

  scottylabs = {
    enable = true;
    project.name = "collar";
    rust.enable = true;
  };

  packages = with pkgs; [
    fd
    git
    git-credential-oauth
    jq
    openbao
    ripgrep
    secretspec
  ];

  scripts.collar.exec = ''
    exec cargo run --quiet --manifest-path "$DEVENV_ROOT/Cargo.toml" -- "$@"
  '';
}
