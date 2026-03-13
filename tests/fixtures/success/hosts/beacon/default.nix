{
  hostName = "beacon";
  system = "x86_64-linux";

  presets = [ "base" ];
  inputModules = [ "fake.direct" ];
}
