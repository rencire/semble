{ lib, discovery, resolution }:
let
  inherit (discovery) discoverProject;
  inherit (resolution) resolveHost resolveImage;
in
{
  mkFlake =
    {
      inputs,
      root,
      overlays ? [ ],
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
            };
          } // extraSpecialArgs;
          modules = resolved.modules ++ [ (resolution.overlayModule overlays) ];
        }
      ) project.hostsByKey;

      images = lib.mapAttrs (
        key: _:
        (resolveImage {
          inherit project key overlays;
        }).build
      ) project.imagesByKey;

      _semble = {
        images = lib.mapAttrs (
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
        ) project.imagesByKey;
      };
    };
}
