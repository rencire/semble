{ nixpkgs }:
let
  lib = nixpkgs.lib;
in
{
  inherit lib;

  mkFlake =
    {
      inputs,
      root,
    }:
    let
      _ = inputs;
      _root = root;
    in
    {
      nixosConfigurations = { };
    };
}
