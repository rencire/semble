{ nixpkgs }:
let
  lib = nixpkgs.lib;
  shared = import ./shared.nix { inherit lib; };
  discovery = import ./discovery.nix { inherit lib shared; };
  resolution = import ./resolution.nix { inherit lib shared; };
  flake = import ./flake.nix { inherit lib discovery resolution; };
in
{
  inherit lib;
  inherit (discovery) discoverProject;
  inherit (resolution) resolveHost resolveImage;
  inherit (flake) mkFlake;
}
