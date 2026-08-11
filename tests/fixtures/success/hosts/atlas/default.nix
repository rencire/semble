{
  hostName = "atlas";
  system = "x86_64-linux";
  type = "physical";

  profiles = [ "core" ];
  modules = [ "extra" "package-marker" ];
  inputModules = [ "fake.direct" ];
  configuration = {
    environment.variables.SEMBLE_INLINE = "from-inline";
  };
  configFile = ./configuration.nix;
}
