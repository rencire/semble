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
  version = "0.4.1";

  src = lib.cleanSource ../..;
  cargoLock = {
    lockFile = ../../Cargo.lock;
    outputHashes = {
      "tianyi-0.1.0" = "sha256-ZPMENlhHXbLtCSqf9Z0Ja59V35FF723sdFCnRY55d+k=";
    };
  };

  nativeBuildInputs = [
    makeWrapper
    nix
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
