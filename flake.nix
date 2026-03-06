{
  description = "Semble development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nix-wrapper-modules = {
      url = "github:BirdeeHub/nix-wrapper-modules";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    confix = {
      url = "github:rencire/confix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.nix-wrapper-modules.follows = "nix-wrapper-modules";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      confix,
      ...
    }:
    let
      lib = nixpkgs.lib;
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      forEachSystem =
        f:
        lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [
                self.overlays.default
              ];
            }
          )
        );
    in
    {
      overlays = import ./nix/overlays;

      packages = forEachSystem (pkgs: import ./nix/packages { inherit pkgs; });

      devShells = forEachSystem (pkgs: import ./nix/devShells { inherit confix pkgs; });
    };
}
