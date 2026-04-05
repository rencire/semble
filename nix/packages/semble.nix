{
  lib,
  nix,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "semble";
  version = "0.4.0";

  src = lib.cleanSource ../..;
  cargoLock = {
    lockFile = ../../Cargo.lock;
    outputHashes = {
      "tianyi-0.1.0" = "sha256-ZPMENlhHXbLtCSqf9Z0Ja59V35FF723sdFCnRY55d+k=";
    };
  };

  nativeBuildInputs = [ nix ];

  meta = {
    description = "Repo-aware host management CLI driven by semble.toml";
    license = lib.licenses.mit;
    maintainers = [ ];
  };
}
