{ confix, entire-cli-nix, configured, pkgs }:
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
      ]
      ++ (with pkgs; [
        cargo
        clippy
        git
        rustc
        rustfmt
      ]);
  };
}
