{ agentSkillsLib
, inputs
, lib
, skillSets
, formats
,
}:
let
  baseNameOf = path:
    let
      match = builtins.match ".*/([^/]+)" path;
    in
    if match == null then path else builtins.elemAt match 0;

  rewriteSkillName = { oldName, newName, text }:
    builtins.replaceStrings
      [ "name: ${oldName}" ]
      [ "name: ${newName}" ]
      text;

  mkPrefixedSkills =
    { source
    , prefix
    , skills
    ,
    }:
    builtins.listToAttrs (
      builtins.map
        (
          path:
          let
            baseName = baseNameOf path;
            prefixedName = "${prefix}-${baseName}";
          in
          {
            name = prefixedName;
            value = {
              from = source;
              inherit path;
              rename = prefixedName;
              transform = { original, ... }:
                builtins.replaceStrings
                  [ "~/.gstack/" ]
                  [ "\${GSTACK_HOME:-\$HOME/.gstack}/" ]
                  (rewriteSkillName {
                    oldName = baseName;
                    newName = prefixedName;
                    text = original;
                  });
            };
          }
        )
        skills
    );

  mkSource = name: cfg:
    {
      subdir = cfg.subdir or ".";
      idPrefix = cfg.idPrefix or name;
    }
    // (if cfg ? input then { path = inputs.${cfg.input}; } else { path = cfg.path; })
    // (if cfg ? filter then { inherit (cfg) filter; } else { });

  sources = builtins.mapAttrs mkSource skillSets;

  explicitSkills = lib.foldlAttrs
    (
      acc: name: cfg:
        acc
        // mkPrefixedSkills {
          source = name;
          prefix = cfg.prefix or name;
          skills = cfg.skills;
        }
    )
    { }
    skillSets;

  catalog = agentSkillsLib.discoverCatalog sources;
  allowlist = agentSkillsLib.allowlistFor {
    inherit catalog sources;
    enable = [ ];
  };

  enabledTargets = builtins.listToAttrs (
    builtins.map
      (
        name:
        {
          inherit name;
          value = agentSkillsLib.defaultLocalTargets.${name} // {
            enable = true;
          };
        }
      )
      formats
  );

  bundle =
    pkgs:
    agentSkillsLib.mkBundle {
      inherit pkgs;
      selection = agentSkillsLib.selectSkills {
        inherit catalog sources allowlist;
        skills = explicitSkills;
      };
    };
in
{
  inherit bundle;
  targets = enabledTargets;
}
