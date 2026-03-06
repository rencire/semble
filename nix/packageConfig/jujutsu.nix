{ pkgs, ... }:
{
  settings.aliases = {
    spr = [
      "util"
      "exec"
      "--"
      "${pkgs.jj-spr}/bin/jj-spr"
    ];
  };
}
