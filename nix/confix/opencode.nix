{ lib, pkgs, ... }:
{
  package = pkgs.llm-agents.opencode;
  settings = {
    "$schema" = "https://opencode.ai/config.json";
    plugin = [
      "opencode-worktree"
      "superpowers@git+https://github.com/obra/superpowers.git"
    ];
    permission = "allow";
    mcp = {
      nixos = {
        type = "local";
        enabled = true;
        command = [
          (lib.getExe pkgs.nix)
          "run"
          "github:utensils/mcp-nixos"
          "--"
        ];
      };
      # NOTE: This server still needs GITHUB_PERSONAL_ACCESS_TOKEN wired in
      # at runtime later, ideally via sops-nix or another secret manager.
      github = {
        type = "local";
        enabled = true;
        command = [
          (lib.getExe pkgs.github-mcp-server)
          "stdio"
        ];
        environment = {
          GITHUB_PERSONAL_ACCESS_TOKEN = "<set later>";
        };
      };
    };
  };
}
