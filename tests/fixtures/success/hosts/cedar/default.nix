{
  hostName = "cedar";
  system = "x86_64-linux";
  type = "physical";
  builder = "fake.lib.nixosSystemFull";

  presets = [ "base" ];
  configuration = {
    environment.variables.CEDAR_INLINE = "enabled";
  };
}
