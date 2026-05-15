{
  enable = true;
  agents = [ "opencode" ];
  settings = {
    telemetry = false;
    strategy_options = {
      checkpoint_remote = {
        provider = "github";
        repo = "rencire/semble";
      };
    };
  };
}
