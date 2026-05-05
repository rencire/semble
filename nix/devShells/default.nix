{
  agentLib,
  bundle,
  confix,
  entire-cli-nix,
  configured,
  localTargets,
  pkgs,
}:
{
  default = pkgs.mkShell {
    packages =
      (confix.lib.configureAsList {
        inherit pkgs;
        configDir = ../packageConfig;
      })
      ++ [
        entire-cli-nix.packages.${pkgs.system}.entire
        configured.opencode
        pkgs.llm-agents.codex
        pkgs.git
      ]
      ++ (with pkgs; [
        cargo
        clippy
        rustc
        rustfmt
      ]);

    shellHook = agentLib.mkShellHook {
      pkgs = pkgs;
      inherit bundle;
      targets = localTargets;
    };
  };
}
