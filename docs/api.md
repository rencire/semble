# Semble API Design

This document defines the standard Semble consumer interface and the internal
normalization model Semble uses to assemble hosts.

It defines what files consumers write, what fields they support, and how
Semble resolves composition and values. Higher-level rationale lives in
`docs/architecture.md`, and shared vocabulary in `docs/terminology.md`.

## Example Project Structure

This is the project structure used in the examples below.

```bash
project_root/
|-- flake.nix
|-- flake.lock
|-- hosts/
|   `-- thor/
|       |-- configuration.nix
|       `-- default.nix
|-- modules/
|   `-- security/
|       `-- sops.nix
|-- presets/
|   `-- security/
|       `-- sopsDefault.nix
`-- profiles/
    `-- base.nix
```

## Consumer Interface

Consumers define files in standard locations.

Semble discovers those files, derives keys from their paths, validates their
contents, and normalizes them into its internal composition model.

The consumer-facing file interface is Semble's primary API. Flake outputs are a
compatibility layer built from that interface.

## Consumer Flake Interface

For v1, the consumer flake calls Semble directly from `outputs`.

Semble is responsible for:

- discovering `hosts/`, `modules/`, `presets/`, and `profiles/`
- validating and normalizing those files
- resolving Semble composition
- returning `nixosConfigurations` for `nixos-rebuild --flake`

The consumer remains responsible for its own `devShells` used with
`nix develop`.

For now, Semble does not need to require or standardize `packages`, `apps`,
`checks`, `templates`, `overlays`, or `darwinConfigurations`.

## Minimal Consumer Flake Example

```nix
{
  description = "Example Semble consumer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    semble = {
      url = "github:your-org/semble";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.semble.lib.mkFlake {
      inherit inputs;
      root = ./.;
    };
}
```

This is the minimal v1 entrypoint:

- the consumer declares `inputs`
- the consumer calls `inputs.semble.lib.mkFlake`
- Semble returns `nixosConfigurations`

## Extended Consumer Flake Example

Consumers can still extend the returned outputs like a normal flake. For
example, a project can add its own `devShells`:

```nix
{
  description = "Example Semble consumer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    semble = {
      url = "github:your-org/semble";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    let
      sembleOutputs = inputs.semble.lib.mkFlake {
        inherit inputs;
        root = ./.;
      };

      system = "x86_64-linux";
      pkgs = import inputs.nixpkgs { inherit system; };
    in
    sembleOutputs // {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          nixd
          alejandra
        ];
      };
    };
}
```

This is the intended v1 extension pattern:

- Semble owns `nixosConfigurations`
- the consumer can add other standard outputs as needed
- `devShells` remain consumer-owned for now

Semble-native concepts such as `hosts`, `modules`, `presets`, and `profiles`
are authored as files in the project tree, not manually assembled as flake
outputs by the consumer.

## Hosts

Each host lives at `hosts/<name>/default.nix`.

```nix
# hosts/thor/default.nix
{
  hostName = "thor";
  system = "x86_64-linux";

  profiles = [ "base" ];
  presets = [ "security.sopsDefault" ];
}
```

Host files express identity and host-local composition intent.

Supported host fields:

- `hostName`: The host identity. Semble also defaults `networking.hostName` to
  this value.
- `system`: The target system string, such as `"x86_64-linux"`.
- `profiles`: A list of profile keys to include.
- `presets`: A list of preset keys to include directly.
- `configFile`: Optional path to a host-local override module. Defaults to
  `./configuration.nix`.

If a host needs a different host-local override file, it can set `configFile`
explicitly:

```nix
# hosts/thor/default.nix
{
  hostName = "thor";
  system = "x86_64-linux";

  profiles = [ "base" ];
  presets = [ "security.sopsDefault" ];

  configFile = ./overrides.nix;
}
```

### Host Override File

If present, the default host-local override file is
`hosts/<name>/configuration.nix`.

```nix
# hosts/thor/configuration.nix
{ lib, pkgs, ... }:
{
  # This is still a regular NixOS module, so standard module arguments are available.
  imports = [ ./hardware-override.nix ];

  # Optional escape hatch for host-local overrides after preset values are applied.
  sb.security.sops.sshKeyFile = "/persist/etc/ssh/ssh_host_ed25519_key";

  # Regular upstream NixOS options can also be set here directly.
  services.openssh.enable = true;

  # hostName defaults networking.hostName, but it can be overridden here.
  networking.hostName = "thor-lab";
}
```

This file is optional. If `configFile` is omitted and `./configuration.nix`
does not exist, Semble should treat it as empty.

## Modules

Modules live under `modules/` and define Semble-managed options, behavior, and
required upstream imports.

```nix
# modules/security/sops.nix
{
  inputs = [ "sops-nix.sops" ];

  options = { lib, ... }: {
    enable = lib.mkEnableOption "Semble SOPS integration";

    sshKeyFile = lib.mkOption {
      type = lib.types.path;
      default = "/etc/ssh/ssh_host_ed25519_key";
    };

    hostKeyType = lib.mkOption {
      type = lib.types.str;
      default = "ed25519";
    };
  };

  config = { lib, cfg, ... }: lib.mkIf cfg.enable {
    sops.age.sshKeyFile = cfg.sshKeyFile;

    services.openssh.hostKeys = [
      {
        path = cfg.sshKeyFile;
        type = cfg.hostKeyType;
      }
    ];
  };
}
```

Module keys are derived from file paths. For example,
`modules/security/sops.nix` becomes `security.sops`.

Supported module fields:

- `key`: Optional explicit key override.
- `inputs`: Upstream module dependencies, resolved by convention.
- `options`: Option schema for the Semble namespace.
- `config`: Behavior that uses those options.

## Presets

Presets live under `presets/`. They compose modules and assign values to
existing module options. They do not define new options.

```nix
# presets/security/sopsDefault.nix
{
  modules = [ "security.sops" ];

  values = {
    sb.security.sops.enable = true;
    sb.security.sops.sshKeyFile = "/etc/ssh/ssh_host_ed25519_key";
    sb.security.sops.hostKeyType = "ed25519";
  };
}
```

Preset keys are derived from file paths. For example,
`presets/security/sopsDefault.nix` becomes `security.sopsDefault`.

Supported preset fields:

- `key`: Optional explicit key override.
- `modules`: A list of module keys to include.
- `values`: A set of values for existing module options.

## Profiles

Profiles live under `profiles/`. They compose presets and do not compose
modules directly.

```nix
# profiles/base.nix
{
  presets = [ "security.sopsDefault" ];
}
```

Profile keys are derived from file paths. For example,
`profiles/base.nix` becomes `base`.

Supported profile fields:

- `key`: Optional explicit key override.
- `presets`: A list of preset keys to include.

## Normalization Semantics

Internally, Semble may normalize these convention-based files through helper
functions such as `mkHost`, `mkModule`, `mkPreset`, and `mkProfile`. Those are
framework implementation details rather than the primary consumer interface.
Consumers do not need to call them directly.

Their job is to:

- Derive and validate keys.
- Normalize file contents into a consistent internal shape.
- Enforce required fields and allowed field sets.
- Lift upstream dependencies into the final import graph.

## Composition Resolution

Semble resolves host structure in this order:

1. Selected `profiles` to presets.
2. Selected and derived presets to modules.
3. Module `inputs` to top-level imports.
4. Assemble the final module import graph.

## Value Application Order

Once the import graph is assembled, Semble applies configuration in this order:

1. Module option defaults and module behavior.
2. Preset `values`.
3. Default host-derived values such as `networking.hostName = hostName`.
4. Host `configFile`, where host-local configuration wins.

## Naming And Key Rules

1. `key` is optional for module, preset, and profile files and is otherwise
   derived from file path.
2. Explicit `key` overrides the derived key.
3. Final keys must be globally unique per kind; conflicts are hard errors.
4. `modules = [ "security.sops" ]` uses short keys with no `modules.` prefix.
5. `inputs = [ "<input>.<module>" ]` resolves by convention as
   `inputs.<input>.nixosModules.<module>`.
6. Unknown module, preset, profile, or input keys are hard errors.
7. Duplicate module, preset, or profile inclusion is a hard error. Semble does
   not deduplicate repeated selections implicitly.

## Deferred Questions

These are intentionally left unspecified for now and are not part of the v1
contract:

1. Whether upstream module resolution needs an explicit escape hatch beyond the
   `inputs.<input>.nixosModules.<module>` convention.
2. What stability guarantees derived keys should have across file moves and
   refactors.
3. Whether Semble should later standardize additional compatibility outputs such
   as `darwinConfigurations`, `checks`, or `nixosModules`.
