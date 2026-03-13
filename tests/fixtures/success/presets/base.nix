{
  modules = [ "branding" ];
  inputModules = [ "fake.bundle" ];

  values = {
    sb.branding.message = "from-preset";
    networking.hostName = "preset-name";
  };
}
