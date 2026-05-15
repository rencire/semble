{ inputs, ... }:
let
  sembleLib = import ./semble-lib { nixpkgs = inputs.nixpkgs; };
in
{
  api = pkgs: pkgs.writeText "semble-api-tests" (
    builtins.toJSON (import ./tests {
      inherit sembleLib;
      nixpkgs = inputs.nixpkgs;
    })
  );
}
