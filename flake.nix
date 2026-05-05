{
  description = "Semble development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nix-wrapper-modules = {
      url = "github:BirdeeHub/nix-wrapper-modules";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    llm-agents = {
      url = "github:numtide/llm-agents.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    entire-cli-nix = {
      url = "github:rencire/entire-cli-nix";
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
      llm-agents,
      entire-cli-nix,
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

      lib = import ./nix/lib { inherit nixpkgs; };

      packages = forEachSystem (pkgs: import ./nix/packages { inherit pkgs; });

      apps = forEachSystem (pkgs: {
        semble = {
          type = "app";
          program = "${pkgs.semble}/bin/semble";
        };
        default = {
          type = "app";
          program = "${pkgs.semble}/bin/semble";
        };
      });

      devShells = forEachSystem (
        pkgs:
        let
          pkgs' = pkgs.extend llm-agents.overlays.shared-nixpkgs;
          configured = confix.lib.configure {
            pkgs = pkgs';
            configDir = ./nix/confix;
          };
        in
        import ./nix/devShells {
          inherit confix entire-cli-nix configured;
          pkgs = pkgs';
        }
      );

      checks = forEachSystem (pkgs: import ./nix/checks {
        inherit pkgs self nixpkgs;
      });
    };
}
