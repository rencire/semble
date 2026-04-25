{ lib, shared }:
let
  inherit (shared)
    assertCondition
    fileError
    joinDot
    makeOrigin
    formatOrigin
    ;

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

  overlayModule = overlays:
    if overlays == [ ] then
      { }
    else
      {
        nixpkgs.overlays = overlays;
      };

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
      inputRefs ? moduleDef.inputs,
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
      }) inputRefs;
      options = lib.setAttrByPath namespacePath optionsValue;
      config = configValue;
    };

  addResolvedItem =
    {
      file,
      label,
      itemType,
      itemKey,
      origin,
      state,
      allowDuplicate,
    }:
    if builtins.hasAttr itemKey state.seen then
      let
        first = builtins.getAttr itemKey state.seen;
      in
      if allowDuplicate then
        state
        // {
          duplicates = state.duplicates ++ [
            {
              key = itemKey;
              type = itemType;
              firstOrigin = first.origin;
              repeatedOrigin = origin;
            }
          ];
        }
      else
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
        duplicates = state.duplicates;
      };

  collectResolvedItems =
    {
      file,
      label,
      itemType,
      selections,
      allowDuplicates ? false,
    }:
    builtins.foldl' (
      state: selection:
      addResolvedItem {
        inherit file label itemType state;
        itemKey = selection.key;
        origin = selection.origin;
        allowDuplicate = allowDuplicates;
      }
    ) { seen = { }; ordered = [ ]; duplicates = [ ]; } selections;

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
      allowDuplicates ? false,
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
      inherit label itemType allowDuplicates;
      selections = presetSelections ++ hostSelections;
    };

  moduleInputPlan =
    {
      moduleDefs,
      seenInputs ? { },
    }:
    let
      stepModule =
        state: moduleDef:
        let
          stepInput =
            inputState: ref:
            if builtins.hasAttr ref inputState.seen then
              inputState
            else
              {
                seen = inputState.seen // { "${ref}" = true; };
                refs = inputState.refs ++ [ ref ];
              };
          inputResult = builtins.foldl' stepInput { seen = state.seen; refs = [ ]; } moduleDef.inputs;
        in
        {
          seen = inputResult.seen;
          planned = state.planned ++ [
            {
              inherit moduleDef;
              inputRefs = inputResult.refs;
            }
          ];
        };
    in
    (builtins.foldl' stepModule { seen = seenInputs; planned = [ ]; } moduleDefs).planned;

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
        allowDuplicates = true;
      };
      moduleDefs = map (selection: requireByKey host.file "module" selection.key project.modulesByKey) explicitModuleSelections.ordered;
      explicitInputSelections = collectExplicitSelections {
        inherit host presetDefs;
        field = "inputModules";
        label = "input module inclusion";
        itemType = "input module";
        allowDuplicates = true;
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
      modulePlans = moduleInputPlan {
        inherit moduleDefs;
        seenInputs = lib.listToAttrs (
          map (selection: lib.nameValuePair selection.key true) explicitInputSelections.ordered
        );
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
          (modulePlan: makeSembleModule {
            inherit (modulePlan) moduleDef inputRefs;
            inherit (project) inputs;
          })
          modulePlans
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
      overlays ? [ ],
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
      imageModulePlans = moduleInputPlan {
        moduleDefs = imageModuleDefs;
        seenInputs = lib.listToAttrs (
          map (selection: lib.nameValuePair selection.key true) explicitImageInputSelections.ordered
        );
      };
      imageConfigFileModule =
        if builtins.pathExists image.configFile then
          image.configFile
        else if image.configFileExplicit then
          fileError image.file "configFile `${toString image.configFile}` does not exist"
        else
          { };
      imageModules =
        map
          (modulePlan: makeSembleModule {
            inherit (modulePlan) moduleDef inputRefs;
            inherit (project) inputs;
          })
          imageModulePlans
        ++ map (
          selection:
          resolveInputRef {
            inputs = project.inputs;
            file = image.file;
            ref = selection.key;
          }
        ) explicitImageInputSelections.ordered
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
        modules = resolvedHost.modules ++ imageModules ++ [ (overlayModule overlays) ];
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
in
{
  inherit
    builderSpecialArgs
    overlayModule
    resolveHost
    resolveImage
    resolveBuilderRef
    ;
}
