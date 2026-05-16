{ nixpkgs }:
let
  lib = nixpkgs.lib;
  shared = import ./shared.nix { inherit lib; };
  discovery = import ./discovery.nix { inherit lib shared; };
  resolution = import ./resolution.nix { inherit lib shared; };
  operatorSsh = import ./operator-ssh.nix { inherit lib shared; };
  flake = import ./flake.nix { inherit lib discovery resolution operatorSsh; };
in
{
  inherit lib;
  inherit (discovery) discoverRepo loadRepo;
  inherit (resolution) resolveHost resolveImage;
  operatorSshArtifacts = args: operatorSsh.operatorSshArtifacts (args // { loadRepo = discovery.loadRepo; });
  inherit (flake) mkFlake;
}
