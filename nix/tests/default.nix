{ sembleLib, nixpkgs }:
let
  lib = nixpkgs.lib;

  successRoot = ../../tests/fixtures/success;
  duplicatePresetRoot = ../../tests/fixtures/duplicate-preset;
  duplicateModuleRoot = ../../tests/fixtures/duplicate-module;
  duplicateInputOverlapRoot = ../../tests/fixtures/duplicate-input-overlap;
  unknownPresetRoot = ../../tests/fixtures/unknown-preset;
  missingConfigRoot = ../../tests/fixtures/missing-config;
  unknownImageHostRoot = ../../tests/fixtures/unknown-image-host;

  testInputs = {
    inherit nixpkgs;
    fake = {
      nixosModules.bundle = { ... }: {
        environment.variables.FAKE_BUNDLE = "enabled";
      };
      nixosModules.direct = { ... }: {
        environment.variables.FAKE_DIRECT = "enabled";
      };
      nixosModules.secrets = { ... }: {
        environment.variables.FAKE_INPUT = "enabled";
      };
    };
  };

  project = sembleLib.discoverProject {
    inputs = testInputs;
    root = successRoot;
  };

  resolved = sembleLib.resolveHost {
    inherit project;
    key = "thor";
  };

  resolvedImage = sembleLib.resolveImage {
    inherit project;
    key = "installer";
  };

  flake = sembleLib.mkFlake {
    inputs = testInputs;
    root = successRoot;
  };

  hostConfig = flake.nixosConfigurations.thor.config;
  lokiConfig = flake.nixosConfigurations.loki.config;
  installerImage = flake.images.installer;

  expectFailure =
    root:
    let
      outcome = builtins.tryEval (
        builtins.deepSeq
          (sembleLib.mkFlake {
            inputs = testInputs;
            inherit root;
          })
          true
      );
    in
    assert (outcome.success == false);
    true;
in
{
  discoveredHosts = assert (builtins.attrNames project.hostsByKey == [ "loki" "thor" ]); true;
  discoveredModules = assert (builtins.attrNames project.modulesByKey == [ "branding" "extra" "security.sops" ]); true;
  discoveredPresets = assert (builtins.attrNames project.presetsByKey == [ "base" "security.sopsDefault" ]); true;
  discoveredImages = assert (builtins.attrNames project.imagesByKey == [ "installer" ]); true;
  presetResolution = assert (map (preset: preset.key) resolved.presetDefs == [ "base" "security.sopsDefault" ]); true;
  moduleResolution = assert (map (moduleDef: moduleDef.key) resolved.moduleDefs == [ "branding" "security.sops" "extra" ]); true;
  imageResolution = assert (resolvedImage.image.host == "thor"); true;
  imageBuildLooksLikeDerivation = assert (builtins.hasAttr "drvPath" installerImage); true;
  hostConfigWins = assert (hostConfig.environment.variables.SEMBLE_MESSAGE == "from-host"); true;
  hostModulesApply = assert (hostConfig.environment.variables.EXTRA_MODULE == "enabled"); true;
  hostNameOrder = assert (hostConfig.networking.hostName == "thor-lab"); true;
  defaultHostNameWins = assert (lokiConfig.networking.hostName == "loki"); true;
  missingDefaultConfigIsEmpty = assert (lokiConfig.environment.variables.SEMBLE_MESSAGE == "from-preset"); true;
  presetValuesApply = assert (hostConfig.services.openssh.hostKeys == [ { path = "/preset/key"; type = "rsa"; } ]); true;
  upstreamInputsApply = assert (hostConfig.environment.variables.FAKE_INPUT == "enabled"); true;
  presetInputModulesApply = assert (hostConfig.environment.variables.FAKE_BUNDLE == "enabled"); true;
  hostInputModulesApply = assert (hostConfig.environment.variables.FAKE_DIRECT == "enabled"); true;
  duplicatePresetFails = expectFailure duplicatePresetRoot;
  duplicateModuleFails = expectFailure duplicateModuleRoot;
  duplicateInputOverlapFails = expectFailure duplicateInputOverlapRoot;
  unknownPresetFails = expectFailure unknownPresetRoot;
  missingConfigFails = expectFailure missingConfigRoot;
  unknownImageHostFails = expectFailure unknownImageHostRoot;
}
