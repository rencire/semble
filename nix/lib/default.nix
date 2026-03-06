{ nixpkgs }:
let
  lib = nixpkgs.lib;

  joinDot = lib.concatStringsSep ".";

  fileError = file: message: throw "${toString file}: ${message}";

  assertCondition = file: condition: message:
    if condition then true else fileError file message;

  assertAttrset = file: value:
    let
      _ = assertCondition file (builtins.isAttrs value) "expected an attribute set";
    in
    value;

  assertString = file: field: value:
    let
      _ = assertCondition file (builtins.isString value) "field `${field}` must be a string";
    in
    value;

  assertListOfStrings = file: field: value:
    let
      _ = assertCondition file (builtins.isList value) "field `${field}` must be a list";
      __ = assertCondition file (builtins.all builtins.isString value) "field `${field}` must contain only strings";
    in
    value;

  assertOptionalPath = file: field: value:
    let
      _ = assertCondition file (builtins.isPath value) "field `${field}` must be a path";
    in
    value;

  assertAttrsOrFunction = file: field: value:
    let
      _ = assertCondition file (builtins.isAttrs value || builtins.isFunction value) "field `${field}` must be an attribute set or function";
    in
    value;

  assertAllowedFields = file: allowed: value:
    let
      extra = builtins.filter (field: !(builtins.elem field allowed)) (builtins.attrNames value);
      _ = assertCondition file (extra == [ ]) "unsupported fields: ${lib.concatStringsSep ", " extra}";
    in
    value;

  toPath = value:
    if builtins.isPath value then
      value
    else
      /. + value;

  stripNixExtension = name: lib.removeSuffix ".nix" name;

  collectTree =
    {
      dir,
      includeFile,
      prefix ? "",
    }:
    if !builtins.pathExists dir then
      [ ]
    else
      let
        entries = builtins.readDir dir;
        names = lib.sort builtins.lessThan (builtins.attrNames entries);
      in
      lib.concatMap (
        name:
        let
          entryType = entries.${name};
          child = dir + "/${name}";
          relativePath = if prefix == "" then name else "${prefix}/${name}";
        in
        if entryType == "directory" then
          collectTree {
            dir = child;
            inherit includeFile;
            prefix = relativePath;
          }
        else if entryType == "regular" && includeFile name relativePath then
          [
            {
              path = child;
              inherit relativePath;
            }
          ]
        else
          [ ]
      ) names;

  deriveKey =
    {
      kind,
      relativePath,
    }:
    let
      parts = lib.splitString "/" relativePath;
      keyParts =
        if kind == "host" then
          lib.init parts
        else
          let
            last = lib.last parts;
          in
          if last == "default.nix" then lib.init parts else (lib.init parts) ++ [ (stripNixExtension last) ];
      _ = assertCondition relativePath (keyParts != [ ]) "unable to derive key";
    in
    joinDot keyParts;

  assertUniqueValues = file: label: values:
    let
      step =
        state: value:
        if builtins.hasAttr value state.seen then
          fileError file "duplicate ${label} `${value}`"
        else
          {
            seen = state.seen // { "${value}" = true; };
            ordered = state.ordered ++ [ value ];
          };
      result = builtins.foldl' step { seen = { }; ordered = [ ]; } values;
    in
    result.ordered;

  assertUniqueItems = kind: items:
    let
      step =
        state: item:
        if builtins.hasAttr item.key state.seen then
          fileError item.file "${kind} key `${item.key}` conflicts with ${builtins.getAttr item.key state.seen}"
        else
          {
            seen = state.seen // { "${item.key}" = toString item.file; };
            ordered = state.ordered ++ [ item ];
          };
      result = builtins.foldl' step { seen = { }; ordered = [ ]; } items;
    in
    result.ordered;

  listToAttrsByKey = items: lib.listToAttrs (map (item: lib.nameValuePair item.key item) items);

  normalizeHost =
    {
      path,
      relativePath,
    }:
    let
      raw = assertAttrset path (import path);
      value = assertAllowedFields path [ "hostName" "system" "profiles" "presets" "configFile" ] raw;
      hostName = assertString path "hostName" (value.hostName or (fileError path "missing required field `hostName`"));
      system = assertString path "system" (value.system or (fileError path "missing required field `system`"));
      profiles = assertUniqueValues path "profile selection" (assertListOfStrings path "profiles" (value.profiles or [ ]));
      presets = assertUniqueValues path "preset selection" (assertListOfStrings path "presets" (value.presets or [ ]));
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
      inherit hostName system profiles presets configFile;
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
      key = value.key or deriveKey {
        kind = "module";
        inherit relativePath;
      };
      _ = assertString path "key" key;
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
      value = assertAllowedFields path [ "key" "modules" "values" ] raw;
      key = value.key or deriveKey {
        kind = "preset";
        inherit relativePath;
      };
      _ = assertString path "key" key;
      modules = assertUniqueValues path "module selection" (assertListOfStrings path "modules" (value.modules or [ ]));
      presetValues = assertAttrset path (value.values or { });
    in
    {
      file = path;
      kind = "preset";
      inherit key modules;
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
      key = value.key or deriveKey {
        kind = "profile";
        inherit relativePath;
      };
      _ = assertString path "key" key;
      presets = assertUniqueValues path "preset selection" (assertListOfStrings path "presets" (value.presets or [ ]));
    in
    {
      file = path;
      kind = "profile";
      inherit key presets;
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
    in
    {
      inherit root inputs hosts modules presets profiles;
      hostsByKey = listToAttrsByKey hosts;
      modulesByKey = listToAttrsByKey modules;
      presetsByKey = listToAttrsByKey presets;
      profilesByKey = listToAttrsByKey profiles;
    };

  requireByKey = file: kind: key: attrs:
    if builtins.hasAttr key attrs then
      builtins.getAttr key attrs
    else
      fileError file "unknown ${kind} `${key}`";

  resolveInputRef =
    {
      inputs,
      file,
      ref,
    }:
    let
      parts = lib.splitString "." ref;
      _ = assertCondition file (builtins.length parts >= 2) "input reference `${ref}` must be `<input>.<module>`";
      inputName = builtins.head parts;
      moduleName = joinDot (lib.tail parts);
      input =
        if builtins.hasAttr inputName inputs then
          builtins.getAttr inputName inputs
        else
          fileError file "unknown input `${inputName}` referenced by `${ref}`";
      modules =
        if input ? nixosModules then
          input.nixosModules
        else
          fileError file "input `${inputName}` does not expose `nixosModules`";
    in
    if builtins.hasAttr moduleName modules then
      builtins.getAttr moduleName modules
    else
      fileError file "input `${inputName}` does not expose nixosModules.${moduleName}";

  overrideValues =
    priority: value:
    if builtins.isAttrs value then
      lib.mapAttrs (_: overrideValues priority) value
    else
      lib.mkOverride priority value;

  makeSembleModule =
    {
      moduleDef,
      inputs,
    }:
    args@{ config, ... }:
    let
      hkPath = [ "hk" ] ++ lib.splitString "." moduleDef.key;
      cfg = lib.attrByPath hkPath { } config;
      optionsValue =
        if builtins.isFunction moduleDef.options then
          moduleDef.options (args // { inherit cfg; })
        else
          moduleDef.options;
      configValue =
        if builtins.isFunction moduleDef.config then
          moduleDef.config (args // { inherit cfg; })
        else
          moduleDef.config;
    in
    {
      imports = map (ref: resolveInputRef {
        inherit inputs ref;
        file = moduleDef.file;
      }) moduleDef.inputs;
      options = lib.setAttrByPath hkPath optionsValue;
      config = configValue;
    };

  resolveHost =
    {
      project,
      key,
    }:
    let
      host = requireByKey project.root "host" key project.hostsByKey;
      profileDefs = map (profileKey: requireByKey host.file "profile" profileKey project.profilesByKey) host.profiles;
      presetKeys = assertUniqueValues host.file "preset inclusion" ((lib.concatMap (profile: profile.presets) profileDefs) ++ host.presets);
      presetDefs = map (presetKey: requireByKey host.file "preset" presetKey project.presetsByKey) presetKeys;
      moduleKeys = assertUniqueValues host.file "module inclusion" (lib.concatMap (preset: preset.modules) presetDefs);
      moduleDefs = map (moduleKey: requireByKey host.file "module" moduleKey project.modulesByKey) moduleKeys;
      hostConfigModule =
        if builtins.pathExists host.configFile then
          host.configFile
        else if host.configFileExplicit then
          fileError host.file "configFile `${toString host.configFile}` does not exist"
        else
          { };
    in
    {
      inherit host profileDefs presetDefs moduleDefs;
      modules =
        map
          (moduleDef: makeSembleModule {
            inherit moduleDef;
            inherit (project) inputs;
          })
          moduleDefs
        ++ map (preset: { config = overrideValues 200 preset.values; }) presetDefs
        ++ [
          {
            config.networking.hostName = lib.mkOverride 150 host.hostName;
          }
          hostConfigModule
        ];
    };

  mkFlake =
    {
      inputs,
      root,
    }:
    let
      project = discoverProject { inherit inputs root; };
    in
    {
      nixosConfigurations = lib.mapAttrs (
        key: host:
        let
          resolved = resolveHost {
            inherit project key;
          };
        in
        inputs.nixpkgs.lib.nixosSystem {
          system = host.system;
          specialArgs = {
            inherit inputs;
            semble = {
              inherit project resolved;
            };
          };
          modules = resolved.modules;
        }
      ) project.hostsByKey;
    };
in
{
  inherit
    discoverProject
    lib
    mkFlake
    resolveHost
    ;
}
