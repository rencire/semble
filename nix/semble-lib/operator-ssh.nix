{ lib, shared }:
let
  inherit (shared) fileError;

  defaultAliasTemplates = [ "admin" "deploy" ];

  templateDefinitions = {
    admin = {
      suffix = "admin";
      user = "admin";
      identityFile = "%d/.ssh/homelab_admin";
    };

    deploy = {
      suffix = "deploy";
      user = "deploy";
      identityFile = "%d/.ssh/homelab_deploy";
    };

    builder = {
      suffix = "builder";
      user = "builder";
      identityFile = "%d/.ssh/homelab_builder";
    };
  };

  templateFor = host: name:
    if builtins.hasAttr name templateDefinitions then
      builtins.getAttr name templateDefinitions
    else
      fileError host.file "unknown operator alias template `${name}`";

  aliasFromTemplate = host: targetHostName: name:
    let
      template = templateFor host name;
    in
    {
      sourceHost = host.key;
      name = "${host.key}-${template.suffix}";
      hostName = targetHostName;
      inherit (template) user identityFile;
    };

  aliasFromExplicit = host: targetHostName: alias:
    {
      sourceHost = host.key;
      name = alias.name or host.key;
      hostName = alias.hostName or targetHostName;
      user = alias.user or (fileError host.file "operator alias must define `user`");
      identityFile = alias.identityFile or (fileError host.file "operator alias must define `identityFile`");
    };

  mergeAliases = aliases:
    builtins.attrValues (builtins.listToAttrs (map (alias: lib.nameValuePair alias.name alias) aliases));

  aliasesForHost = host:
    let
      operator = host.operator or { };
      targetHostName = operator.hostName or host.hostName;
      templateNames = (operator.aliasTemplates or defaultAliasTemplates) ++ (operator.extraAliasTemplates or [ ]);
      templateAliases = map (aliasFromTemplate host targetHostName) templateNames;
      explicitAliases = map (aliasFromExplicit host targetHostName) (operator.aliases or [ ]);
    in
    mergeAliases (templateAliases ++ explicitAliases);

  operatorServers = repo:
    lib.filter (host: (host.operator.role or null) == "server") (builtins.attrValues repo.hostsByKey);

  renderAlias = alias: ''
    Host ${alias.name}
      HostName ${alias.hostName}
      User ${alias.user}
      IdentityFile ${alias.identityFile}
      IdentitiesOnly yes
  '';

  readPublicKey = repo: host:
    let
      path = repo.paths.sshHostKeysDir + "/${host.key}/ssh_host_ed25519_key.pub";
      line = lib.removeSuffix "\n" (builtins.readFile path);
      parts = lib.filter (part: part != "") (lib.splitString " " line);
      keyType = builtins.elemAt parts 0;
      key = builtins.elemAt parts 1;
    in
    {
      inherit keyType key;
      raw = "${keyType} ${key}";
    };

  knownHostForHost = repo: aliases: host:
    let
      publicKey = readPublicKey repo host;
      hostAliases = lib.filter (alias: alias.sourceHost == host.key) aliases;
      names = lib.unique (lib.sort builtins.lessThan ([ (host.operator.hostName or host.hostName) ] ++ map (alias: alias.name) hostAliases));
    in
    {
      sourceHost = host.key;
      inherit names publicKey;
      line = "${lib.concatStringsSep "," names} ${publicKey.raw}";
    };

  operatorSshArtifacts =
    { repo ? null
    , root ? null
    , loadRepo ? null
    ,
    }:
    let
      resolvedRepo =
        if repo != null then
          repo
        else if root != null && loadRepo != null then
          loadRepo { inherit root; }
        else
          throw "operatorSshArtifacts requires `repo`, or `root` plus `loadRepo`";
      servers = operatorServers resolvedRepo;
      aliases = lib.concatMap aliasesForHost servers;
      knownHosts = map (knownHostForHost resolvedRepo aliases) servers;
    in
    {
      inherit aliases knownHosts;
      sshConfigText = lib.concatStringsSep "\n" (map renderAlias aliases) + "\n";
      knownHostsText = lib.concatStringsSep "\n" (map (entry: entry.line) knownHosts) + "\n";
    };
in
{
  inherit operatorSshArtifacts;
}
