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
      lib.nixosSystemFull =
        args:
        nixpkgs.lib.nixosSystem (
          args
          // {
            modules = args.modules ++ [
              {
                environment.variables.FAKE_SYSTEM_BUILDER = "nixosSystemFull";
              }
            ];
          }
        );
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
    key = "atlas";
  };

  resolvedImage = sembleLib.resolveImage {
    inherit project;
    key = "installer";
  };

  flake = sembleLib.mkFlake {
    inputs = testInputs;
    root = successRoot;
  };

  hostConfig = flake.nixosConfigurations.atlas.config;
  beaconConfig = flake.nixosConfigurations.beacon.config;
  cedarConfig = flake.nixosConfigurations.cedar.config;
  installerImage = flake.images.installer;
  installerConfig = resolvedImage.system.config;

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
  discoveredHosts = assert (builtins.attrNames project.hostsByKey == [ "atlas" "beacon" "cedar" ]); true;
  discoveredModules = assert (builtins.attrNames project.modulesByKey == [ "branding" "extra" "security.sops" ]); true;
  discoveredPresets = assert (builtins.attrNames project.presetsByKey == [ "base" "security.sopsDefault" ]); true;
  discoveredImages = assert (builtins.attrNames project.imagesByKey == [ "installer" ]); true;
  presetResolution = assert (map (preset: preset.key) resolved.presetDefs == [ "base" "security.sopsDefault" ]); true;
  moduleResolution = assert (map (moduleDef: moduleDef.key) resolved.moduleDefs == [ "branding" "security.sops" "extra" ]); true;
  imageResolution = assert (resolvedImage.image.host == "atlas"); true;
  imageBuildLooksLikeDerivation = assert (builtins.hasAttr "drvPath" installerImage); true;
  hostConfigWins = assert (hostConfig.environment.variables.SEMBLE_MESSAGE == "from-host"); true;
  hostInlineConfigurationApplies = assert (hostConfig.environment.variables.SEMBLE_INLINE == "from-inline"); true;
  hostInlineAndConfigFileMerge = assert (cedarConfig.environment.variables.CEDAR_INLINE == "enabled"); true;
  hostModulesApply = assert (hostConfig.environment.variables.EXTRA_MODULE == "enabled"); true;
  hostNameOrder = assert (hostConfig.networking.hostName == "atlas-lab"); true;
  defaultHostNameWins = assert (beaconConfig.networking.hostName == "beacon"); true;
  missingDefaultConfigIsEmpty = assert (beaconConfig.environment.variables.SEMBLE_MESSAGE == "from-preset"); true;
  presetValuesApply = assert (hostConfig.services.openssh.hostKeys == [ { path = "/preset/key"; type = "rsa"; } ]); true;
  upstreamInputsApply = assert (hostConfig.environment.variables.FAKE_INPUT == "enabled"); true;
  presetInputModulesApply = assert (hostConfig.environment.variables.FAKE_BUNDLE == "enabled"); true;
  hostInputModulesApply = assert (hostConfig.environment.variables.FAKE_DIRECT == "enabled"); true;
  customHostBuilderApplies = assert (cedarConfig.environment.variables.FAKE_SYSTEM_BUILDER == "nixosSystemFull"); true;
  imageInlineConfigurationApplies = assert (installerConfig.environment.variables.IMAGE_INLINE == "enabled"); true;
  imageConfigFileApplies = assert (installerConfig.environment.variables.IMAGE_FILE == "enabled"); true;
  duplicatePresetFails = expectFailure duplicatePresetRoot;
  duplicateModuleFails = expectFailure duplicateModuleRoot;
  duplicateInputOverlapFails = expectFailure duplicateInputOverlapRoot;
  unknownPresetFails = expectFailure unknownPresetRoot;
  missingConfigFails = expectFailure missingConfigRoot;
  unknownImageHostFails = expectFailure unknownImageHostRoot;
}
