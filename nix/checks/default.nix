{ pkgs, self }:
{
  api-scaffold = pkgs.writeText "semble-api-scaffold" (
    builtins.toJSON {
      hasMkFlake = builtins.isFunction self.lib.mkFlake;
    }
  );
}
