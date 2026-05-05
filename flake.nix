{
  description = "Semble development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flakelight = {
      url = "github:accelbread/flakelight";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    agent-skills = {
      url = "github:Kyure-A/agent-skills-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    personal-skills = {
      url = "github:rencire/agent-skills";
      flake = false;
    };
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
    }@inputs:
    let
      lib = nixpkgs.lib;
      agentLib = inputs."agent-skills".lib."agent-skills";
      sources = {
        shared = {
          path = inputs."personal-skills";
          subdir = "skills";
        };
      };
      enabledSkills = [
        "dev-loop"
        "doc-table-of-contents"
        "nix-repo"
        "public-repo-readiness"
        "vcs"
      ];
      catalog = agentLib.discoverCatalog sources;
      allowlist = agentLib.allowlistFor {
        inherit catalog sources;
        enable = enabledSkills;
      };
      selection = agentLib.selectSkills {
        inherit catalog allowlist sources;
        skills = { };
      };
      localTargets = {
        agents = agentLib.defaultLocalTargets.agents // {
          enable = true;
        };
        claude = agentLib.defaultLocalTargets.claude // {
          enable = false;
        };
      };
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
          bundle = agentLib.mkBundle {
            pkgs = pkgs';
            inherit selection;
          };
          configured = confix.lib.configure {
            pkgs = pkgs';
            configDir = ./nix/confix;
          };
        in
        import ./nix/devShells {
          inherit
            agentLib
            bundle
            confix
            entire-cli-nix
            configured
            localTargets
            ;
          pkgs = pkgs';
        }
      );

      checks = forEachSystem (
        pkgs:
        import ./nix/checks {
          inherit pkgs self nixpkgs;
        }
      );
    };
}
