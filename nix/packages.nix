{ ... }:
{
  default = { rustPlatform, lib, coreutils, e2fsprogs, makeWrapper, nix, openssh, pv, util-linux, xz, zstd, ... }:
    rustPlatform.buildRustPackage {
      pname = "semble";
      version = "0.7.0";
      src = lib.cleanSource ../.;
      cargoLock.lockFile = ../Cargo.lock;
      nativeBuildInputs = [ coreutils e2fsprogs makeWrapper nix openssh util-linux ];
      postInstall = ''
        wrapProgram $out/bin/semble \
          --prefix PATH : ${lib.makeBinPath [ coreutils e2fsprogs openssh pv util-linux xz zstd ]}
      '';
      meta = {
        description = "Repo-aware host management CLI driven by semble.toml";
        license = lib.licenses.mit;
      };
    };
}
