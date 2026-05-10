{
  coreutils,
  lib,
  makeWrapper,
  nix,
  openssh,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "semble";
  version = "0.5.0";

  src = lib.cleanSource ../..;
  cargoLock = {
    lockFile = ../../Cargo.lock;
  };

  nativeBuildInputs = [
    makeWrapper
    nix
    openssh
  ];

  postInstall = ''
    wrapProgram $out/bin/semble \
      --prefix PATH : ${lib.makeBinPath [
        coreutils
        openssh
      ]}
  '';

  meta = {
    description = "Repo-aware host management CLI driven by semble.toml";
    license = lib.licenses.mit;
    maintainers = [ ];
  };
}
