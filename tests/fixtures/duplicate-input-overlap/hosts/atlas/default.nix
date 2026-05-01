{
  hostName = "atlas";
  system = "x86_64-linux";
  type = "physical";

  modules = [ "security.sops" ];
  inputModules = [ "fake.secrets" ];
}
