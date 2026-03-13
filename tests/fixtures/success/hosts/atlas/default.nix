{
  hostName = "atlas";
  system = "x86_64-linux";

  profiles = [ "core" ];
  modules = [ "extra" ];
  inputModules = [ "fake.direct" ];
}
