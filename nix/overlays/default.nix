{
  default = final: prev: {
    jj-spr = final.callPackage ../packages/jj-spr.nix { };
    semble = final.callPackage ../packages/semble.nix { };
  };
}
