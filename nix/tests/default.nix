{ sembleLib, nixpkgs }:
let
  lib = nixpkgs.lib;

  successRoot = ../../tests/fixtures/success;
  duplicatePresetRoot = ../../tests/fixtures/duplicate-preset;
  duplicateModuleRoot = ../../tests/fixtures/duplicate-module;
  unknownPresetRoot = ../../tests/fixtures/unknown-preset;
  missingConfigRoot = ../../tests/fixtures/missing-config;

  testInputs = {
    inherit nixpkgs;
    fake = {
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

  flake = sembleLib.mkFlake {
    inputs = testInputs;
    root = successRoot;
  };

  hostConfig = flake.nixosConfigurations.thor.config;

  expectFailure =
    root:
    let
      outcome = builtins.tryEval (
        let
          failedFlake = sembleLib.mkFlake {
            inputs = testInputs;
            inherit root;
          };
        in
        failedFlake.nixosConfigurations.thor.config.networking.hostName
      );
    in
    assert (outcome.success == false);
    true;
in
{
  discoveredHosts = assert (builtins.attrNames project.hostsByKey == [ "thor" ]); true;
  discoveredModules = assert (builtins.attrNames project.modulesByKey == [ "branding" "security.sops" ]); true;
  discoveredPresets = assert (builtins.attrNames project.presetsByKey == [ "base" "security.sopsDefault" ]); true;
  presetResolution = assert (map (preset: preset.key) resolved.presetDefs == [ "base" "security.sopsDefault" ]); true;
  moduleResolution = assert (map (moduleDef: moduleDef.key) resolved.moduleDefs == [ "branding" "security.sops" ]); true;
  hostConfigWins = assert (hostConfig.environment.variables.SEMBLE_MESSAGE == "from-host"); true;
  hostNameOrder = assert (hostConfig.networking.hostName == "thor-lab"); true;
  presetValuesApply = assert (hostConfig.services.openssh.hostKeys == [ { path = "/preset/key"; type = "rsa"; } ]); true;
  upstreamInputsApply = assert (hostConfig.environment.variables.FAKE_INPUT == "enabled"); true;
  duplicatePresetFails = expectFailure duplicatePresetRoot;
  duplicateModuleFails = expectFailure duplicateModuleRoot;
  unknownPresetFails = expectFailure unknownPresetRoot;
  missingConfigFails = expectFailure missingConfigRoot;
}
