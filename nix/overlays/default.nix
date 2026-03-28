{
  default = final: prev: {
    semble = final.callPackage ../packages/semble.nix { };
  };
}
