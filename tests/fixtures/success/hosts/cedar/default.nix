{
  hostName = "cedar";
  system = "x86_64-linux";
  builder = "fake.lib.nixosSystemFull";

  presets = [ "base" ];
  configuration = {
    environment.variables.CEDAR_INLINE = "enabled";
  };
}
