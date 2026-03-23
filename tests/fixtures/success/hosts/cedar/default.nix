{
  hostName = "cedar";
  system = "x86_64-linux";
  builder = "fake.lib.nixosSystemFull";

  presets = [ "base" ];
}
