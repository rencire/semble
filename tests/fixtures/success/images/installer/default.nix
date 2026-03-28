{
  host = "atlas";
  format = "raw";
  efiSupport = true;
  configFile = ./configuration.nix;
  configuration = {
    environment.variables.IMAGE_INLINE = "enabled";
  };
}
