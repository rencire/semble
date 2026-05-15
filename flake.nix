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
      url = "github:rencire/nix-wrapper-modules/feat/wofr-wrapper";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    llm-agents = {
      url = "github:numtide/llm-agents.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    entire-cli-flake = {
      url = "github:rencire/entire-cli-flake";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flakelight.follows = "flakelight";
      inputs.confix.follows = "confix";
      inputs.nix-wrapper-modules.follows = "nix-wrapper-modules";
      inputs.llm-agents.follows = "llm-agents";
      inputs.agent-skills.follows = "agent-skills";
      inputs.rencire-skills.follows = "personal-skills";
    };
    confix = {
      url = "github:rencire/confix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flakelight.follows = "flakelight";
      inputs.nix-wrapper-modules.follows = "nix-wrapper-modules";
    };
    gstack = {
      url = "github:garrytan/gstack";
      flake = false;
    };
  };

  outputs = { flakelight, ... }@inputs:
    flakelight ./. {
      inherit inputs;
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
    };
}
