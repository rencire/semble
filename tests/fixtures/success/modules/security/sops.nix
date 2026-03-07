{
  inputs = [ "fake.secrets" ];

  options = { lib, ... }: {
    enable = lib.mkEnableOption "Semble SOPS integration";

    sshKeyFile = lib.mkOption {
      type = lib.types.str;
      default = "/etc/ssh/default";
    };

    hostKeyType = lib.mkOption {
      type = lib.types.str;
      default = "ed25519";
    };
  };

  config = { lib, cfg, ... }: lib.mkIf cfg.enable {
    services.openssh.enable = true;
    services.openssh.hostKeys = [
      {
        path = cfg.sshKeyFile;
        type = cfg.hostKeyType;
      }
    ];
  };
}
