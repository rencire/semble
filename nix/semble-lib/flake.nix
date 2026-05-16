{ lib, discovery, resolution, operatorSsh }:
let
  inherit (resolution) resolveHost resolveImage;
in
{
  mkFlake =
    { inputs
    , root
    , overlays ? [ ]
    ,
    }:
    let
      project = discovery.loadRepo { inherit inputs root; };
      repoSshArtifacts = operatorSsh.operatorSshArtifacts { repo = project; };
      buildHost = key: host:
        let
          resolved = resolveHost {
            inherit project key;
          };
          builder = resolution.resolveBuilderRef {
            inputs = inputs;
            file = host.file;
            ref = host.builder;
          };
          extraSpecialArgs = resolution.builderSpecialArgs {
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
              repo = project;
              operatorSshArtifacts = repoSshArtifacts;
            };
          } // extraSpecialArgs;
          modules = resolved.modules ++ [ (resolution.overlayModule overlays) ];
        };
      nixosHosts = lib.filterAttrs (_: host: !lib.hasSuffix "-darwin" host.system) project.hostsByKey;
      darwinHosts = lib.filterAttrs (_: host: lib.hasSuffix "-darwin" host.system) project.hostsByKey;
    in
    {
      nixosConfigurations = lib.mapAttrs buildHost nixosHosts;
      darwinConfigurations = lib.mapAttrs buildHost darwinHosts;

      images = lib.mapAttrs
        (
          key: _:
            (resolveImage {
              inherit project key overlays;
            }).build
        )
        project.imagesByKey;

      _semble = {
        images = lib.mapAttrs
          (
            key: _:
              let
                resolvedImage = resolveImage {
                  inherit project key;
                };
              in
              {
                sourceHost = resolvedImage.image.sourceHost;
                buildOutput = resolvedImage.image.buildOutput;
                prepare = resolvedImage.image.prepare;
              }
          )
          project.imagesByKey;
        hosts = lib.mapAttrs
          (
            key: host:
              {
                inherit (host) system;
                type = host.type;
                provisionTarget = host.provisionTarget or null;
                operator = host.operator or { };
              }
          )
          project.hostsByKey;
      };
    };
}
