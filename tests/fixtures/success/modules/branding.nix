{
  options = { lib, ... }: {
    message = lib.mkOption {
      type = lib.types.str;
      default = "unset";
    };
  };

  config = { cfg, ... }: {
    environment.variables.SEMBLE_MESSAGE = cfg.message;
  };
}
