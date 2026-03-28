{ pkgs, ... }:
{
  environment.variables.IMAGE_FILE = "enabled";
  system.build.altImage = pkgs.writeText "installer-image" "ok";
}
