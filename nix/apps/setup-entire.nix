{ pkgs, inputs, ... }:
let
  inherit (pkgs) lib;
  system = pkgs.stdenv.hostPlatform.system;
  cfg = import ../config/setup-entire-config.nix;
  entire = inputs.entire-cli-flake.packages.${system}.default;
  settingsJson = builtins.toJSON ({ enabled = true; } // cfg.settings);
  script = pkgs.writeShellScript "entire-init" ''
        set -euo pipefail
        export PATH="${lib.makeBinPath [ entire ]}:$PATH"
        ${if cfg.enable then ''
          mkdir -p .entire
          cat > .entire/settings.json << 'JSONEOF'
          ${settingsJson}
    JSONEOF

          ${lib.concatStringsSep "\n" (map (agent: ''
            echo "-> Enabling entire for ${agent}..."
            entire enable -y --agent "${agent}"
          '') cfg.agents)}

          echo "entire-init complete"
        '' else ''
          echo "-> Disabling entire in project..."
          entire disable --project
          echo "entire-init complete (disabled)"
        ''}
  '';
in
{
  type = "app";
  program = "${script}";
}
