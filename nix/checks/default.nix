{ pkgs, self, nixpkgs }:
{
  api = pkgs.writeText "semble-api-tests" (
    builtins.toJSON (import ../tests {
      sembleLib = self.lib;
      inherit nixpkgs;
    })
  );
}
