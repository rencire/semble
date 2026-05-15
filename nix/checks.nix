{ inputs, ... }:
let
  theLib = import ./semble-lib { nixpkgs = inputs.nixpkgs; };
in
{
  api = pkgs: pkgs.writeText "semble-api-tests" (
    builtins.toJSON (import ./tests {
      sembleLib = theLib;
      nixpkgs = inputs.nixpkgs;
    })
  );
}
