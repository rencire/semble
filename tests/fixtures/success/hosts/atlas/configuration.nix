{ lib, pkgs, ... }:
{
  networking.hostName = "atlas-lab";
  sb.branding.message = "from-host";
  environment.systemPackages = lib.optionals (pkgs ? overlay-marker) [ pkgs.overlay-marker ];
  system.stateVersion = "24.11";
}
