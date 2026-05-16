{ lib, shared }:
let
  inherit (shared)
    assertAllowedFields
    assertAttrset
    assertAttrsOrFunction
    assertCondition
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
    { path
    , relativePath
    ,
    }:
    let
      raw = assertAttrset path (import path);
      value = assertAllowedFields path [ "hostName" "system" "builder" "type" "provisionTarget" "operator" "profiles" "presets" "modules" "inputModules" "configFile" "configuration" ] raw;
      hostName = assertString path "hostName" (value.hostName or (fileError path "missing required field `hostName`"));
      system = assertString path "system" (value.system or (fileError path "missing required field `system`"));
      builder = assertString path "builder" (
        value.builder or (
          if lib.hasSuffix "-darwin" system
          then "nix-darwin.lib.darwinSystem"
          else "nixpkgs.lib.nixosSystem"
        )
      );
      hostType = assertString path "type" (value.type or (fileError path "missing required field `type`"));
      _typeCheck =
        assertCondition
          path
          (builtins.elem hostType [ "physical" "microvm" ])
          "field `type` must be one of `physical` or `microvm`";
      provisionTarget =
        if value ? provisionTarget then
          assertString path "provisionTarget" value.provisionTarget
        else
          null;
      operator = assertOptionalAttrset path "operator" (value.operator or { });
      _provisionCheck =
        assertCondition
          path
          (
            if hostType == "microvm" then
              provisionTarget != null
            else
              provisionTarget == null
          )
          (
            if hostType == "microvm" then
              "missing required field `provisionTarget` for microvm host"
            else
              "field `provisionTarget` is only supported for `type = \"microvm\"`"
          );
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
      type = hostType;
      inherit provisionTarget operator;
      configFileExplicit = value ? configFile;
    };

  normalizeModule =
    { path
    , relativePath
    ,
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
    { path
    , relativePath
    ,
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
    { path
    , relativePath
    ,
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
    { path
    , relativePath
    ,
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
    { root
    , name
    , dir ? root + "/${name}"
    , includeFile
    , normalize
    ,
    }:
    assertUniqueItems name (
      map normalize (
        collectTree {
          inherit dir;
          inherit includeFile;
        }
      )
    );

  discoverRepo =
    { root
    , inputs
    ,
    }:
    let
      sembleConfig =
        if builtins.pathExists (root + "/semble.toml") then
          builtins.fromTOML (builtins.readFile (root + "/semble.toml"))
        else
          { paths = { }; };
      paths = {
        hostsDir = root + "/${sembleConfig.paths.hosts_dir or "hosts"}";
        sshHostKeysDir = root + "/${sembleConfig.paths.ssh_host_keys_dir or "ssh_host_keys"}";
        diskKeysDir = root + "/${sembleConfig.paths.disk_keys_dir or "disk_keys"}";
        hostTemplateDir = root + "/${sembleConfig.paths.host_template_dir or "hosts/_template"}";
        sopsConfigFile = root + "/${sembleConfig.paths.sops_config_file or ".sops.yaml"}";
        networkSecretsFile = root + "/${sembleConfig.paths.network_secrets_file or "secrets/network.yaml"}";
      };
      hosts = discoverKind {
        inherit root;
        name = "hosts";
        dir = paths.hostsDir;
        includeFile = fileName: relativePath:
          fileName == "default.nix" &&
          builtins.length (lib.splitString "/" relativePath) == 2;
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
      inherit root inputs paths hosts modules presets profiles images;
      hostsByKey = listToAttrsByKey hosts;
      modulesByKey = listToAttrsByKey modules;
      presetsByKey = listToAttrsByKey presets;
      profilesByKey = listToAttrsByKey profiles;
      imagesByKey = listToAttrsByKey images;
    };
in
{
   inherit discoverRepo;
   loadRepo = { root, inputs ? { } }: discoverRepo { inherit root inputs; };
}
