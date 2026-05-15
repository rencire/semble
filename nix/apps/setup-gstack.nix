{ pkgs, inputs, ... }:
let
  inherit (pkgs) lib;
  gstackCfg = import ../config/setup-gstack-config.nix;
  script = pkgs.writeShellScript "gstack-init" ''
    set -euo pipefail
    export PATH="${pkgs.bun}/bin:$PATH"
    SRC="${inputs.gstack}"
    DEST=".gstack/repos/gstack"

    if [ ! -d "$DEST" ]; then
      mkdir -p "$(dirname "$DEST")"
      cp -r "$SRC" "$DEST"
      chmod -R u+w "$DEST"
    fi

    cd "$DEST"

    echo "-> Installing gstack for claude..."
    ./setup --prefix

    ${lib.concatStringsSep "\n" (builtins.map (host: ''
      echo "-> Installing gstack for ${host}..."
      ./setup --prefix --host "${host}"
    '') gstackCfg.extraHosts)}

    ${lib.optionalString gstackCfg.team.enable ''
      echo "-> Enabling team mode..."
      ./setup --prefix --team

      echo "-> Adding gstack to CLAUDE.md..."
      ./bin/gstack-team-init "${gstackCfg.team.mode}"
    ''}

    echo "gstack-init complete"
  '';
in
{
  type = "app";
  program = "${script}";
}
