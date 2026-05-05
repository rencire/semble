# nix/confix/opencode.nix
{ lib, pkgs, ... }:
{
  package = pkgs.opencode;
  settings = {
    "$schema" = "https://opencode.ai/config.json";
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
      github = {
        type = "local";
        enabled = false;
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
