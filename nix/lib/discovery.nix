{ lib, shared }:
let
  inherit (shared)
    assertAllowedFields
    assertAttrset
    assertAttrsOrFunction
    assertListOfStrings
    assertOptionalAttrset
    assertOptionalPath
    assertString
    assertUniqueItems
    assertUniqueValues
    collectTree
    deriveKey
    fileError
    listToAttrsByKey
    toPath
    ;

  normalizeHost =
    {
      path,
      relativePath,
    }:
    let
      raw = assertAttrset path (import path);
      value = assertAllowedFields path [ "hostName" "system" "builder" "profiles" "presets" "modules" "inputModules" "configFile" "configuration" ] raw;
      hostName = assertString path "hostName" (value.hostName or (fileError path "missing required field `hostName`"));
      system = assertString path "system" (value.system or (fileError path "missing required field `system`"));
      builder = assertString path "builder" (value.builder or "nixpkgs.lib.nixosSystem");
      profiles = assertUniqueValues path "profile selection" (assertListOfStrings path "profiles" (value.profiles or [ ]));
      presets = assertUniqueValues path "preset selection" (assertListOfStrings path "presets" (value.presets or [ ]));
      modules = assertUniqueValues path "module selection" (assertListOfStrings path "modules" (value.modules or [ ]));
      inputModules = assertUniqueValues path "input module selection" (assertListOfStrings path "inputModules" (value.inputModules or [ ]));
      configuration =
        if value ? configuration then
          assertAttrsOrFunction path "configuration" value.configuration
        else
          { };
      configFile =
        if value ? configFile then
          assertOptionalPath path "configFile" value.configFile
        else
          toPath "${builtins.dirOf (toString path)}/configuration.nix";
    in
    {
      file = path;
      kind = "host";
      key = deriveKey {
        kind = "host";
        inherit relativePath;
      };
      inherit hostName system builder profiles presets modules inputModules configuration configFile;
      configFileExplicit = value ? configFile;
    };

  normalizeModule =
    {
      path,
      relativePath,
    }:
    let
      raw = assertAttrset path (import path);
      value = assertAllowedFields path [ "key" "inputs" "options" "config" ] raw;
      key =
        if value ? key then
          assertString path "key" value.key
        else
          deriveKey {
            kind = "module";
            inherit relativePath;
          };
      inputs = assertUniqueValues path "module input" (assertListOfStrings path "inputs" (value.inputs or [ ]));
      options = assertAttrsOrFunction path "options" (value.options or { });
      config = assertAttrsOrFunction path "config" (value.config or { });
    in
    {
      file = path;
      kind = "module";
      inherit key inputs options config;
    };

  normalizePreset =
    {
      path,
      relativePath,
    }:
    let
      raw = assertAttrset path (import path);
      value = assertAllowedFields path [ "key" "modules" "inputModules" "values" ] raw;
      key =
        if value ? key then
          assertString path "key" value.key
        else
          deriveKey {
            kind = "preset";
            inherit relativePath;
          };
      modules = assertUniqueValues path "module selection" (assertListOfStrings path "modules" (value.modules or [ ]));
      inputModules = assertUniqueValues path "input module selection" (assertListOfStrings path "inputModules" (value.inputModules or [ ]));
      presetValues = assertAttrset path (value.values or { });
    in
    {
      file = path;
      kind = "preset";
      inherit key modules inputModules;
      values = presetValues;
    };

  normalizeProfile =
    {
      path,
      relativePath,
    }:
    let
      raw = assertAttrset path (import path);
      value = assertAllowedFields path [ "key" "presets" ] raw;
      key =
        if value ? key then
          assertString path "key" value.key
        else
          deriveKey {
            kind = "profile";
            inherit relativePath;
          };
      presets = assertUniqueValues path "preset selection" (assertListOfStrings path "presets" (value.presets or [ ]));
    in
    {
      file = path;
      kind = "profile";
      inherit key presets;
    };

  normalizeImage =
    {
      path,
      relativePath,
    }:
    let
      raw = assertAttrset path (import path);
      value = assertAllowedFields path [ "sourceHost" "configFile" "configuration" "modules" "inputModules" "buildOutput" "prepare" ] raw;
      sourceHost = assertString path "sourceHost" (value.sourceHost or (fileError path "missing required field `sourceHost`"));
      configuration =
        if value ? configuration then
          assertAttrsOrFunction path "configuration" value.configuration
        else
          { };
      modules = assertUniqueValues path "module selection" (assertListOfStrings path "modules" (value.modules or [ ]));
      inputModules = assertUniqueValues path "input module selection" (assertListOfStrings path "inputModules" (value.inputModules or [ ]));
      configFile =
        if value ? configFile then
          assertOptionalPath path "configFile" value.configFile
        else
          toPath "${builtins.dirOf (toString path)}/configuration.nix";
      buildOutput =
        if value ? buildOutput then
          assertString path "buildOutput" value.buildOutput
        else
          "config.system.build.image";
      prepare =
        if value ? prepare then
          let
            prepareValue = assertOptionalAttrset path "prepare" value.prepare;
            _ = assertAllowedFields path [ "partitionLabel" ] prepareValue;
          in
          {
            partitionLabel =
              if prepareValue ? partitionLabel then
                assertString path "prepare.partitionLabel" prepareValue.partitionLabel
              else
                null;
          }
        else
          { partitionLabel = null; };
    in
    {
      file = path;
      kind = "image";
      key = deriveKey {
        kind = "image";
        inherit relativePath;
      };
      inherit sourceHost configuration configFile modules inputModules buildOutput prepare;
      configFileExplicit = value ? configFile;
    };

  discoverKind =
    {
      root,
      name,
      includeFile,
      normalize,
    }:
    assertUniqueItems name (
      map normalize (
        collectTree {
          dir = root + "/${name}";
          inherit includeFile;
        }
      )
    );

  discoverProject =
    {
      root,
      inputs,
    }:
    let
      hosts = discoverKind {
        inherit root;
        name = "hosts";
        includeFile = fileName: _: fileName == "default.nix";
        normalize = normalizeHost;
      };
      modules = discoverKind {
        inherit root;
        name = "modules";
        includeFile = fileName: _: lib.hasSuffix ".nix" fileName;
        normalize = normalizeModule;
      };
      presets = discoverKind {
        inherit root;
        name = "presets";
        includeFile = fileName: _: lib.hasSuffix ".nix" fileName;
        normalize = normalizePreset;
      };
      profiles = discoverKind {
        inherit root;
        name = "profiles";
        includeFile = fileName: _: lib.hasSuffix ".nix" fileName;
        normalize = normalizeProfile;
      };
      images = discoverKind {
        inherit root;
        name = "images";
        includeFile = fileName: _: fileName == "default.nix";
        normalize = normalizeImage;
      };
    in
    {
      inherit root inputs hosts modules presets profiles images;
      hostsByKey = listToAttrsByKey hosts;
      modulesByKey = listToAttrsByKey modules;
      presetsByKey = listToAttrsByKey presets;
      profilesByKey = listToAttrsByKey profiles;
      imagesByKey = listToAttrsByKey images;
    };
in
{
  inherit discoverProject;
}
