{
  lib,
  rustPlatform,
  fetchFromGitHub,
  openssl,
  zlib,
  pkg-config,
  git,
  jujutsu,
}:

rustPlatform.buildRustPackage rec {
  pname = "jj-spr";
  version = "1.3.7";

  src = fetchFromGitHub {
    owner = "LucioFranco";
    repo = "jj-spr";
    rev = "v${version}";
    hash = "sha256-SM0tW4urrOCMw9BANVoh65G7zAMwtJw+F0LDInQHGxo=";
  };

  cargoHash = "sha256-3pBP8ZgKiKnE6fK4a9IVR67br33ktsmB5ofwTyy95wA=";

  doCheck = false;

  buildInputs = [
    openssl
    zlib
  ];

  nativeBuildInputs = [
    pkg-config
    git
    jujutsu
  ];

  meta = with lib; {
    description = "Jujutsu subcommand for submitting pull requests for individual, amendable, rebaseable commits to GitHub";
    homepage = "https://github.com/LucioFranco/jj-spr";
    license = licenses.mit;
    maintainers = [ ];
    mainProgram = "jj-spr";
  };
}
