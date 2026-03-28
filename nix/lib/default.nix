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

  assertOptionalAttrset = file: field: value:
    let
      _ = assertCondition file (builtins.isAttrs value) "field `${field}` must be an attribute set";
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
      value = assertAllowedFields path [ "host" "sourceHost" "format" "efiSupport" "configFile" "configuration" "modules" "inputModules" "buildOutput" "prepare" ] raw;
      sourceHost =
        if value ? sourceHost && value ? host then
          let
            sourceHostValue = assertString path "sourceHost" value.sourceHost;
            hostValue = assertString path "host" value.host;
            _ = assertCondition path (sourceHostValue == hostValue) "fields `sourceHost` and `host` must match when both are present";
          in
          sourceHostValue
        else if value ? sourceHost then
          assertString path "sourceHost" value.sourceHost
        else if value ? host then
          assertString path "host" value.host
        else
          fileError path "missing required field `sourceHost`";
      format =
        if value ? format then
          let
            formatValue = assertString path "format" value.format;
            _ = assertCondition path (builtins.elem formatValue [ "raw" ]) "field `format` must be one of: raw";
          in
          formatValue
        else
          null;
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
      efiSupport =
        if value ? efiSupport then
          let
            enabled = value.efiSupport;
            __ = assertCondition path (builtins.isBool enabled) "field `efiSupport` must be a boolean";
          in
          enabled
        else
          false;
    in
    {
      file = path;
      kind = "image";
      key = deriveKey {
        kind = "image";
        inherit relativePath;
      };
      inherit sourceHost format efiSupport configuration configFile modules inputModules buildOutput prepare;
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

  resolveBuilderRef =
    {
      inputs,
      file,
      ref,
    }:
    let
      parts = lib.splitString "." ref;
      _ = assertCondition file (builtins.length parts >= 3) "builder reference `${ref}` must be `<input>.<path>.<function>`";
      inputName = builtins.head parts;
      attrPath = lib.tail parts;
      input =
        if builtins.hasAttr inputName inputs then
          builtins.getAttr inputName inputs
        else
          fileError file "unknown input `${inputName}` referenced by builder `${ref}`";
    in
    if lib.hasAttrByPath attrPath input then
      lib.getAttrFromPath attrPath input
    else
      fileError file "input `${inputName}` does not expose `${joinDot attrPath}`";

  resolveAttrRef =
    {
      file,
      root,
      ref,
      label,
    }:
    let
      parts = lib.splitString "." ref;
      _ = assertCondition file (parts != [ ]) "${label} `${ref}` must not be empty";
    in
    if lib.hasAttrByPath parts root then
      lib.getAttrFromPath parts root
    else
      fileError file "${label} `${ref}` does not exist";

  builderSpecialArgs =
    {
      inputs,
      ref,
    }:
    if lib.hasPrefix "nixos-raspberrypi." ref then
      { nixos-raspberrypi = inputs.nixos-raspberrypi; }
    else
      { };

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
      namespacePath = [ "sb" ] ++ lib.splitString "." moduleDef.key;
      cfg = lib.attrByPath namespacePath { } config;
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
      options = lib.setAttrByPath namespacePath optionsValue;
      config = configValue;
    };

  makeOrigin = kind: key: file: {
    inherit kind key file;
  };

  formatOrigin = origin: "${origin.kind} `${origin.key}` (${toString origin.file})";

  addResolvedItem =
    {
      file,
      label,
      itemType,
      itemKey,
      origin,
      state,
    }:
    if builtins.hasAttr itemKey state.seen then
      let
        first = builtins.getAttr itemKey state.seen;
      in
      fileError file "duplicate ${label} `${itemKey}` via ${formatOrigin first.origin}; repeated from ${formatOrigin origin}"
    else
      {
        seen = state.seen // {
          "${itemKey}" = {
            inherit origin;
          };
        };
        ordered = state.ordered ++ [
          {
            key = itemKey;
            type = itemType;
            inherit origin;
          }
        ];
      };

  collectResolvedItems =
    {
      file,
      label,
      itemType,
      selections,
    }:
    builtins.foldl' (
      state: selection:
      addResolvedItem {
        inherit file label itemType state;
        itemKey = selection.key;
        origin = selection.origin;
      }
    ) { seen = { }; ordered = [ ]; } selections;

  collectPresetSelections =
    {
      host,
      profileDefs,
    }:
    let
      profileSelections = lib.concatMap (
        profile:
        map
          (presetKey: {
            key = presetKey;
            origin = makeOrigin "profile" profile.key profile.file;
          })
          profile.presets
      ) profileDefs;
      hostSelections = map
        (presetKey: {
          key = presetKey;
          origin = makeOrigin "host" host.key host.file;
        })
        host.presets;
    in
    collectResolvedItems {
      file = host.file;
      label = "preset inclusion";
      itemType = "preset";
      selections = profileSelections ++ hostSelections;
    };

  collectExplicitSelections =
    {
      host,
      presetDefs,
      field,
      label,
      itemType,
    }:
    let
      presetSelections = lib.concatMap (
        preset:
        map
          (itemKey: {
            key = itemKey;
            origin = makeOrigin "preset" preset.key preset.file;
          })
          preset.${field}
      ) presetDefs;
      hostSelections = map
        (itemKey: {
          key = itemKey;
          origin = makeOrigin "host" host.key host.file;
        })
        host.${field};
    in
    collectResolvedItems {
      file = host.file;
      inherit label itemType;
      selections = presetSelections ++ hostSelections;
    };

  assertNoOverlapWithOrigins =
    {
      file,
      label,
      explicitSelections,
      transitiveSelections,
    }:
    let
      explicitByKey = lib.listToAttrs (map (selection: lib.nameValuePair selection.key selection) explicitSelections);
      overlaps = builtins.filter (selection: builtins.hasAttr selection.key explicitByKey) transitiveSelections;
      _ = builtins.map (
        overlap:
        let
          explicit = builtins.getAttr overlap.key explicitByKey;
        in
        fileError file "duplicate ${label} `${overlap.key}` via ${formatOrigin explicit.origin}; repeated from ${formatOrigin overlap.origin}"
      ) overlaps;
    in
    true;

  resolveHost =
    {
      project,
      key,
    }:
    let
      host = requireByKey project.root "host" key project.hostsByKey;
      profileDefs = map (profileKey: requireByKey host.file "profile" profileKey project.profilesByKey) host.profiles;
      presetSelections = collectPresetSelections {
        inherit host profileDefs;
      };
      presetDefs = map (selection: requireByKey host.file "preset" selection.key project.presetsByKey) presetSelections.ordered;
      explicitModuleSelections = collectExplicitSelections {
        inherit host presetDefs;
        field = "modules";
        label = "module inclusion";
        itemType = "module";
      };
      moduleDefs = map (selection: requireByKey host.file "module" selection.key project.modulesByKey) explicitModuleSelections.ordered;
      explicitInputSelections = collectExplicitSelections {
        inherit host presetDefs;
        field = "inputModules";
        label = "input module inclusion";
        itemType = "input module";
      };
      transitiveInputSelections = lib.concatMap (
        moduleDef:
        map
          (ref: {
            key = ref;
            origin = makeOrigin "module" moduleDef.key moduleDef.file;
          })
          moduleDef.inputs
      ) moduleDefs;
      _ = assertNoOverlapWithOrigins {
        file = host.file;
        label = "input module inclusion";
        explicitSelections = explicitInputSelections.ordered;
        inherit transitiveInputSelections;
      };
      hostConfigFileModule =
        if builtins.pathExists host.configFile then
          host.configFile
        else if host.configFileExplicit then
          fileError host.file "configFile `${toString host.configFile}` does not exist"
        else
          { };
    in
    {
      inherit host profileDefs presetDefs moduleDefs;
      presetSelections = presetSelections.ordered;
      explicitModuleSelections = explicitModuleSelections.ordered;
      explicitInputSelections = explicitInputSelections.ordered;
      transitiveInputSelections = transitiveInputSelections;
      modules =
        map
          (moduleDef: makeSembleModule {
            inherit moduleDef;
            inherit (project) inputs;
          })
          moduleDefs
        ++ map (
          selection:
          resolveInputRef {
            inputs = project.inputs;
            file = host.file;
            ref = selection.key;
          }
        ) explicitInputSelections.ordered
        ++ map (preset: { config = overrideValues 200 preset.values; }) presetDefs
        ++ [
          {
            config.networking.hostName = lib.mkOverride 150 host.hostName;
          }
          host.configuration
          hostConfigFileModule
        ];
    };

  resolveImage =
    {
      project,
      key,
    }:
    let
      image = requireByKey project.root "image" key project.imagesByKey;
      resolvedHost = resolveHost {
        inherit project;
        key = image.sourceHost;
      };
      explicitImageModuleSelections = collectResolvedItems {
        file = image.file;
        label = "image module inclusion";
        itemType = "module";
        selections = map
          (moduleKey: {
            key = moduleKey;
            origin = makeOrigin "image" image.key image.file;
          })
          image.modules;
      };
      imageModuleDefs = map (selection: requireByKey image.file "module" selection.key project.modulesByKey) explicitImageModuleSelections.ordered;
      explicitImageInputSelections = collectResolvedItems {
        file = image.file;
        label = "image input module inclusion";
        itemType = "input module";
        selections = map
          (ref: {
            key = ref;
            origin = makeOrigin "image" image.key image.file;
          })
          image.inputModules;
      };
      transitiveImageInputSelections = lib.concatMap (
        moduleDef:
        map
          (ref: {
            key = ref;
            origin = makeOrigin "module" moduleDef.key moduleDef.file;
          })
          moduleDef.inputs
      ) imageModuleDefs;
      _ = assertNoOverlapWithOrigins {
        file = image.file;
        label = "image input module inclusion";
        explicitSelections = explicitImageInputSelections.ordered;
        transitiveSelections = transitiveImageInputSelections;
      };
      imageConfigFileModule =
        if builtins.pathExists image.configFile then
          image.configFile
        else if image.configFileExplicit then
          fileError image.file "configFile `${toString image.configFile}` does not exist"
        else
          { };
      legacyImageModule =
        lib.optional (image.format != null || image.efiSupport) (
          {
            imports = [ "${project.inputs.nixpkgs}/nixos/modules/virtualisation/disk-image.nix" ];
            image = lib.optionalAttrs (image.format != null) { inherit (image) format; } // {
              inherit (image) efiSupport;
            };
          }
        );
      imageModules =
        map
          (moduleDef: makeSembleModule {
            inherit moduleDef;
            inherit (project) inputs;
          })
          imageModuleDefs
        ++ map (
          selection:
          resolveInputRef {
            inputs = project.inputs;
            file = image.file;
            ref = selection.key;
          }
        ) explicitImageInputSelections.ordered
        ++ legacyImageModule
        ++ [
          image.configuration
          imageConfigFileModule
        ];
      builder = resolveBuilderRef {
        inputs = project.inputs;
        file = resolvedHost.host.file;
        ref = resolvedHost.host.builder;
      };
      extraSpecialArgs = builderSpecialArgs {
        inputs = project.inputs;
        ref = resolvedHost.host.builder;
      };
      system = builder {
        system = resolvedHost.host.system;
        specialArgs = {
          inherit (project) inputs;
          semble = {
            inherit project;
            resolved = resolvedHost;
            image = image;
          };
        } // extraSpecialArgs;
        modules = resolvedHost.modules ++ imageModules;
      };
    in
    {
      inherit image resolvedHost system;
      moduleDefs = imageModuleDefs;
      explicitInputSelections = explicitImageInputSelections.ordered;
      transitiveInputSelections = transitiveImageInputSelections;
      modules = resolvedHost.modules ++ imageModules;
      build = resolveAttrRef {
        file = image.file;
        root = system;
        ref = image.buildOutput;
        label = "buildOutput";
      };
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
      builder = resolveBuilderRef {
        inputs = inputs;
        file = host.file;
        ref = host.builder;
      };
      extraSpecialArgs = builderSpecialArgs {
        inherit inputs;
        ref = host.builder;
      };
    in
        builder {
          system = host.system;
          specialArgs = {
            inherit inputs;
            semble = {
              inherit project resolved;
            };
          } // extraSpecialArgs;
          modules = resolved.modules;
        }
      ) project.hostsByKey;

      images = lib.mapAttrs (
        key: _:
        (resolveImage {
          inherit project key;
        }).build
      ) project.imagesByKey;
    };
in
{
  inherit
    discoverProject
    lib
    mkFlake
    resolveHost
    resolveImage
    ;
}
