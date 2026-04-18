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
|   `-- atlas/
|       |-- configuration.nix
|       `-- default.nix
|-- images/
|   `-- installer/
|       `-- default.nix
|-- modules/
|   |-- security/
|   |   `-- sops.nix
|   `-- virtualization/
|       `-- microvm-host.nix
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

The intended layering model is:

- `inputModules`: raw upstream escape hatch
- `modules`: local capability layer
- `presets`: reusable bundles and defaults
- `profiles`: broad reusable baselines

Consumers should usually start at `inputModules` or `modules`, and only promote
upward when repetition appears.

## Consumer Flake Interface

For v1, the consumer flake calls Semble directly from `outputs`.

Semble is responsible for:

- discovering `hosts/`, `modules/`, `presets/`, and `profiles/`
- validating and normalizing those files
- resolving Semble composition across profiles, presets, modules, and raw input module escape hatches
- returning `nixosConfigurations` for `nixos-rebuild --flake`
- returning `images` for bootable artifact builds such as `nix build .#images.installer`

The consumer remains responsible for its own `devShells` used with
`nix develop`.

For v0.3, Semble standardizes `images/` as a first-class consumer convention in
addition to host composition, and it adds an explicit `overlays` argument to
`mkFlake` for host and image evaluation. It still does not require or
standardize `packages`, `apps`, `checks`, `templates`, or
`darwinConfigurations`.

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
- Semble returns `nixosConfigurations` and `images`

## Project Overlays

Consumers may pass project overlays directly to `mkFlake`:

```nix
{
  outputs = inputs:
    inputs.semble.lib.mkFlake {
      inherit inputs;
      root = ./.;
      overlays = [
        (import ./overlays/default.nix)
      ];
    };
}
```

Semble applies these overlays to the `pkgs` instance used for:

- `nixosConfigurations`
- `images`

This makes overlay-provided packages available in host and image modules via
`pkgs`.

Semble does not apply these overlays automatically to consumer-defined flake
outputs such as `packages` or `devShells`; those remain consumer-owned.

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
# hosts/atlas/default.nix
{
  hostName = "atlas";
  system = "x86_64-linux";

  profiles = [ "base" ];
  presets = [ "security.sopsDefault" ];
  modules = [ "virtualization.microvm-host" ];
  inputModules = [ "microvm.host" ];
  ssh.aliases = [ ];
}
```

Host files express identity and host-local composition intent.

Supported host fields:

- `hostName`: The host identity. Semble also defaults `networking.hostName` to
  this value.
- `system`: The target system string, such as `"x86_64-linux"`.
- `builder`: Optional attr-path string selecting the system constructor. Defaults
  to `nixpkgs.lib.nixosSystem`. This allows alternate host backends such as
  `nixos-raspberrypi.lib.nixosSystemFull`.
- `profiles`: A list of profile keys to include.
- `presets`: A list of preset keys to include directly.
- `modules`: A list of local Semble module keys to include directly for this host.
- `inputModules`: A list of raw input module references to include directly for this host. This is the raw upstream layer for cases where a local Semble abstraction is not needed yet.
- `configFile`: Optional path to a host-local override module. Defaults to
  `./configuration.nix`.
- `configuration`: Optional inline host-local module content.
- `ssh.aliases`: Optional list overriding generated SSH client aliases for this host.
  - if omitted, consumers may apply repo-wide defaults from their Nix-owned SSH alias configuration
  - if set to `[]`, no SSH aliases are generated for the host
  - if set to a list, consumers should use that list exactly

Example custom SSH alias override:

```nix
{
  hostName = "genesis";
  system = "x86_64-linux";

  presets = [ "installer" ];

  ssh.aliases = [
    {
      name = "genesis";
      user = "root";
      identityFile = "~/.ssh/installer_key";
    }
  ];
}
```

### Layer Selection Rule

Hosts should start at one of these two layers:

- `inputModules`, when they want to consume an upstream NixOS module directly
- `modules`, when they already want a local Semble capability name and option surface

They should move upward only when repetition appears:

- promote repeated module or `inputModules` combinations into a `preset`
- promote repeated preset combinations into a `profile`

Semble does not require a local module or preset just to hide a single upstream
import. If the only shared fact is "this host needs `microvm.host`", keep that
as `inputModules` until a real local capability abstraction emerges.

If a host needs a different host-local override file, it can set `configFile`
explicitly:

```nix
# hosts/atlas/default.nix
{
  hostName = "atlas";
  system = "x86_64-linux";

  profiles = [ "base" ];
  presets = [ "security.sopsDefault" ];
  modules = [ "virtualization.microvm-host" ];

  configFile = ./overrides.nix;
}
```

Hosts may also define inline configuration directly in `default.nix`:

```nix
{
  hostName = "atlas";
  system = "x86_64-linux";

  configuration = {
    services.openssh.enable = true;
  };
}
```

Hosts that need a non-default constructor can override `builder`:

```nix
{
  hostName = "vishnu";
  system = "aarch64-linux";
  builder = "nixos-raspberrypi.lib.nixosSystemFull";

  inputModules = [
    "nixos-raspberrypi.raspberry-pi-02.base"
    "nixos-raspberrypi.usb-gadget-ethernet"
  ];
}
```

### Host Override File

If present, the default host-local override file is
`hosts/<name>/configuration.nix`.

```nix
# hosts/atlas/configuration.nix
{ lib, pkgs, ... }:
{
  # This is still a regular NixOS module, so standard module arguments are available.
  imports = [ ./hardware-override.nix ];

  # Optional escape hatch for host-local overrides after preset values are applied.
  sb.security.sops.sshKeyFile = "/persist/etc/ssh/ssh_host_ed25519_key";

  # Regular upstream NixOS options can also be set here directly.
  services.openssh.enable = true;

  # hostName defaults networking.hostName, but it can be overridden here.
  networking.hostName = "atlas-lab";
}
```

`configuration` and `configFile` may both be present. Semble includes both as
modules. If `configFile` is omitted and `./configuration.nix` does not exist,
Semble should treat it as empty.

## Images

Each image lives at `images/<name>/default.nix`.

```nix
# images/installer/default.nix
{
  sourceHost = "atlas";
  buildOutput = "config.system.build.image";

  prepare.partitionLabel = "nixos";

  configuration = {
    image = {
      format = "raw";
      efiSupport = true;
    };
  };
}
```

Image files express packaging intent for a host-defined system. They do not
replace host definitions or duplicate host composition.

Supported image fields:

- `sourceHost`: The host key to package into a boot artifact.
- `modules`: Optional local Semble module keys to include only for this image.
- `inputModules`: Optional raw input module references to include only for this
  image.
- `buildOutput`: Attr-path string selecting the final build artifact from the
  evaluated image system, for example `config.system.build.image` or
  `config.system.build.sdImage`.
- `prepare.partitionLabel`: Optional Semble image-prepare metadata for SSH host
  key injection.
- `configuration`: Optional inline image-local module content.
- `configFile`: Optional path to an image-local module file. Defaults to
  `./configuration.nix`.

### Image Resolution Rule

Images are always resolved from an existing host definition. Semble first
resolves the referenced source host through the normal host pipeline, then
appends the image-specific packaging module stack.

That means images own artifact packaging, while hosts continue to own system
behavior.

### Image Outputs

Semble exports resolved images under the flake `images` output:

```bash
nix build .#images.installer
```

The resulting value is a derivation for the image artifact, not a second host
definition.

Images may also define inline configuration directly in `default.nix`:

```nix
{
  sourceHost = "installer";
  buildOutput = "config.system.build.image";

  configuration = {
    image.efiSupport = true;
  };
}
```

`configuration` and `configFile` may both be present. Semble includes both as
modules. If `configFile` is omitted and `./configuration.nix` does not exist,
Semble treats it as empty.

### Image Prepare Metadata

When a consumer uses `semble image prepare`, image-specific preparation metadata
lives in the image definition itself.

Current supported prepare fields:

- `prepare.partitionLabel`: filesystem label of the partition that should
  receive SSH host keys during injection

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

Modules are the local capability layer. They are appropriate when the project
wants:

- a stable local capability name
- a Semble-managed option surface under `sb.*`
- reusable behavior beyond a one-off upstream import

Modules are not required just because an upstream import exists. If there is no
meaningful local API yet, `inputModules` is the simpler starting point.

## Presets

Presets live under `presets/`. They compose modules and assign reusable
default values to existing module options. They do not define new options.

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
- `inputModules`: A list of raw input module references to include.
- `values`: A set of values for existing module options.

Presets are the bundle/default layer. They are appropriate only once several
module choices, `inputModules`, or default values recur together.

Preset-level `inputModules` are valid reusable composition. Semble does not
force every repeated upstream bundle to become a local module abstraction
immediately. If there is no repeated bundle yet, stay at `inputModules` or
`modules`.

## Profiles

Profiles live under `profiles/`. They compose presets and define broad
baselines. They do not compose modules directly.

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

Profiles are the broad baseline layer. They should represent repeated machine
baselines, not ad hoc per-host composition.

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
2. Presets contribute explicit `modules`, `inputModules`, and `values`.
3. Hosts contribute explicit `modules` and `inputModules`.
4. Included Semble modules contribute transitive `inputs`.
5. Assemble the final module import graph.

## Value Application Order

Once the import graph is assembled, Semble applies configuration in this order:

1. Module option defaults and module behavior.
2. Preset `values`.
3. Default host-derived values such as `networking.hostName = hostName`.
4. Host `configFile`, where host-local configuration wins.

Hosts should usually start at one of two layers:

- `inputModules`, when the project wants to use an upstream module directly.
- `modules`, when the project already wants a local Semble capability name and interface.

From there:

- repeated module or `inputModules` combinations can be promoted into `presets`
- repeated preset combinations can be promoted into `profiles`

This is the intended progression:

1. direct upstream need -> `inputModules`
2. local capability/API -> `modules`
3. repeated bundle/defaults -> `presets`
4. repeated baseline -> `profiles`

## Naming And Key Rules

1. `key` is optional for module, preset, and profile files and is otherwise
   derived from file path.
2. Explicit `key` overrides the derived key.
3. Final keys must be globally unique per kind; conflicts are hard errors.
4. `modules = [ "security.sops" ]` uses short keys with no `modules.` prefix.
5. `inputs = [ "<input>.<module>" ]` resolves by convention as
   `inputs.<input>.nixosModules.<module>`.
6. Unknown module, preset, profile, or input keys are hard errors.
7. Duplicate values inside a single selection list are hard errors.
8. Duplicate inclusion remains a hard error for ownership layers such as
   `presets` and `profiles`.
9. Repeated `modules` and `inputModules` across composition sources are
   dependency declarations. Semble deduplicates them by key using first-wins
   ordering so each preset can declare its own requirements without implicitly
   depending on another preset.

## Deferred Questions

These are intentionally left unspecified for now and are not part of the v1
contract:

1. What stability guarantees derived keys should have across file moves and
   refactors.
2. Whether Semble should later standardize additional compatibility outputs such
   as `darwinConfigurations`, `checks`, or `nixosModules`.
