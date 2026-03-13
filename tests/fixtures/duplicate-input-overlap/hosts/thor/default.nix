{
  hostName = "thor";
  system = "x86_64-linux";

  modules = [ "security.sops" ];
  inputModules = [ "fake.secrets" ];
}
