{
  modules = [ "security.sops" ];

  values = {
    hk.security.sops.enable = true;
    hk.security.sops.sshKeyFile = "/preset/key";
    hk.security.sops.hostKeyType = "rsa";
  };
}
