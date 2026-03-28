{
  sourceHost = "atlas";
  modules = [ "extra" ];
  inputModules = [ "fake.direct" ];
  buildOutput = "config.system.build.altImage";
  configFile = ./configuration.nix;
  prepare.partitionLabel = "NIXOS_SD";
  configuration = {
    environment.variables.IMAGE_INLINE = "enabled";
  };
}
