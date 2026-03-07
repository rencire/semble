{
  modules = [ "security.sops" ];

  values = {
    sb.security.sops.enable = true;
    sb.security.sops.sshKeyFile = "/preset/key";
    sb.security.sops.hostKeyType = "rsa";
  };
}
