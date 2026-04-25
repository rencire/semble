{ lib }:
let
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

  makeOrigin = kind: key: file: {
    inherit kind key file;
  };

  formatOrigin = origin: "${origin.kind} `${origin.key}` (${toString origin.file})";
in
{
  inherit
    joinDot
    fileError
    assertCondition
    assertAttrset
    assertString
    assertListOfStrings
    assertOptionalPath
    assertAttrsOrFunction
    assertOptionalAttrset
    assertAllowedFields
    toPath
    stripNixExtension
    collectTree
    deriveKey
    assertUniqueValues
    assertUniqueItems
    listToAttrsByKey
    makeOrigin
    formatOrigin
    ;
}
