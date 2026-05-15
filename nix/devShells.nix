{ inputs, ... }:
let
  agentSkillsLib = inputs.agent-skills.lib."agent-skills";
  agentSkillsConfig = import ./config/agent-skills-config.nix;
  agentBundle = import ./agent-bundle.nix {
    inherit agentSkillsLib inputs;
    lib = inputs.nixpkgs.lib;
    inherit (agentSkillsConfig) skillSets formats;
  };
in
{
  default = pkgs:
    let
      configured = inputs.confix.lib.configure {
        inherit pkgs;
        configDir = ./confix;
      };
    in
    {
      packages = with pkgs; [
        entire
        configured.opencode
        llm-agents.codex
        git
        cargo
        clippy
        rustc
        rustfmt
      ];
      env.GSTACK_HOME = ".gstack";
      shellHook = agentSkillsLib.mkShellHook {
        inherit pkgs;
        bundle = agentBundle.bundle pkgs;
        targets = agentBundle.targets;
      };
    };
}
