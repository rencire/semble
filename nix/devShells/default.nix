{ confix, pkgs }:
{
  default = pkgs.mkShell {
    packages =
      (confix.lib.configureAsList {
        inherit pkgs;
        configDir = ../packageConfig;
      })
      ++ (with pkgs; [
        cargo
        clippy
        rustc
        rustfmt
      ]);
  };
}
