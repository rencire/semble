{
  config = { pkgs, ... }: {
    environment.variables.PACKAGE_MARKER =
      if pkgs ? "overlay-marker" then pkgs."overlay-marker".name else "base";
  };
}
