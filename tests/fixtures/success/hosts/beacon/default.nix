{
  hostName = "beacon";
  system = "x86_64-linux";
  type = "microvm";
  provisionTarget = "thor-admin";

  presets = [ "base" ];
  inputModules = [ "fake.direct" ];
}
