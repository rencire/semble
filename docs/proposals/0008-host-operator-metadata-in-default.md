# Proposal: Generate operator SSH config in Semble

Status: `accepted`

## Table Of Contents

- [Summary](#summary)
- [Background](#background)
- [Problem](#problem)
- [Proposed Change](#proposed-change)
- [Source Of Truth](#source-of-truth)
- [Role Semantics](#role-semantics)
- [Alias Defaults](#alias-defaults)
- [Explicit Alias Behavior](#explicit-alias-behavior)
- [Semble Nix Library API](#semble-nix-library-api)
- [Flake Integration](#flake-integration)
- [Semble CLI Behavior](#semble-cli-behavior)
- [Consumer Repo Responsibility](#consumer-repo-responsibility)
- [Why Keep Installation In Host Config](#why-keep-installation-in-host-config)
- [Rensemble Migration](#rensemble-migration)
- [Verification Plan](#verification-plan)

## Summary

Move operator SSH alias generation out of `rensemble`'s Nix modules and into
Semble's Nix library helpers.

Host manifests in `default.nix` remain the source of truth for operator
metadata. Semble exposes pure Nix helpers that read that metadata, derive the
operator SSH artifacts, and return rendered SSH config and known-hosts text.
Client hosts install that returned text as part of their normal declarative SSH
configuration.

This proposal supersedes the earlier draft direction where the consumer repo's
Nix module rendered aliases directly from host metadata, as well as the later
cache-file handoff design where the Semble CLI wrote untracked generated files
for Nix to read.

## Background

Today, Semble has a few places that describe a host:

- `default.nix` holds the host manifest (`hostName`, `system`, `type`,
  `presets`, etc.)
- `configuration.nix` holds the actual machine config
- `presets` are preset configurations of either nixos configuration, or semble
module configuration.
- `semble modules` are modules that act as facades over nixos modules.


## Problem

Semble currently supports generating SSH aliases for client machines so they
can connect to server machines with stable, memorable names.

Today, those alias settings live as Nix configuration under the `operator`
Semble module. In practice, that means they can be declared in more than one
place:

- directly in `hosts/<host>/configuration.nix`
- in files imported by `configuration.nix`
- in presets that eventually contribute Nix configuration

The current implementation has a bug because it assumes those settings always
live in `configuration.nix` itself. That assumption is false. Once the settings
move into imported modules or presets, the alias generator no longer has a
single file it can read directly.

To compensate, the current design pushes the consumer-side operator module
toward the wrong job: it has to crawl the repo's `./hosts` tree and fully
evaluate host configurations just to recover operator alias metadata.

That is the wrong abstraction boundary. Semble modules are meant to define
configuration, not to discover metadata by evaluating multiple host
configurations across the repo.

The fix is to stop treating operator alias data as derived Nix configuration
state and instead treat it as Semble-owned host metadata. Once that metadata is
promoted into the host manifest, Semble can read it directly and expose a pure
artifact renderer for consumer host modules.


## Proposed Change

The proposed design follows directly from that boundary:

- host manifests describe metadata
- Semble resolves and renders aliases
- consumer hosts only install and include the rendered text

Add an `operator` field to `default.nix` and treat it as the source of truth
for operator SSH metadata.

Semble will read that metadata through its Nix library and return the final SSH
config and known-hosts text. Consumer modules install that text into the target
system; they do not crawl manifests or reimplement alias rendering rules.

Example server host:

```nix
{
  hostName = "example-server";
  system = "x86_64-linux";
  type = "physical";

  operator = {
    role = "server";
    extraAliasTemplates = [ "builder" ];
  };

  modules = [
    "network.ethernet"
    "network.wifi"
  ];

  presets = [
    "base"
    "hardware-tools"
    "vpn-access"
    "builder-server"
  ];
}
```

Example client host:

```nix
{
  hostName = "example-client";
  system = "aarch64-darwin";
  type = "physical";

  operator = {
    role = "client";
  };

  configuration = {
    sb.ssh.aliases.enable = true;
  };

  modules = [
    "shared-user-packages"
    "system.utilities"
  ];
}
```

## Source Of Truth

`default.nix` is the source of truth for operator metadata.

Semble should not read `configuration.nix` to decide which hosts publish
operator aliases. Consumer repos should not infer alias behavior from evaluated
host config internals.

## Role Semantics

`operator.role` is currently a single enum value.

Supported meanings:

- `server`: this host contributes alias metadata to rendered operator SSH artifacts
- `client`: this host is expected to consume rendered operator SSH artifacts

Semble behavior:

- `operator.role = "server"`: include this host in alias generation
- `operator.role = "client"`: identify the host as an operator workstation for
  workflow assistance and documentation

## Alias Defaults

For `operator.role = "server"`, Semble generates these default aliases:

- `<host>-admin`
- `<host>-deploy`

Additional aliases may be requested explicitly.

`builder` aliases are opt-in via metadata, for example:

```nix
operator.extraAliasTemplates = [ "builder" ];
```

Hosts may suppress the default template aliases by setting:

```nix
operator.aliasTemplates = [ ];
```

An empty `aliasTemplates` list means "generate no default template aliases for
this host." Explicit aliases still apply. This is the intended shape for
special hosts that do not expose the normal `admin` / `deploy` users.

## Explicit Alias Behavior

Hosts may also define explicit aliases.

Example:

```nix
operator.aliases = [
  {
    name = "bootstrap-host";
    user = "root";
    identityFile = "%d/.ssh/bootstrap_installer";
  }
];
```

Merge behavior:

1. Template aliases are populated first.
2. Explicit aliases are merged on top.
3. If an explicit alias uses the same alias name as a template alias, the
   explicit alias overrides the template alias.
4. The final rendered config contains exactly one stanza per alias name.

This makes explicit aliases a true override layer instead of a purely additive
mechanism.

## Semble Nix Library API

Semble should expose pure Nix helpers that derive operator SSH artifacts from
normalized Semble repo metadata.

The recommended public entrypoint is:

```nix
inputs.semble.lib.operatorSshArtifacts {
  repo = semble.repo;
}
```

For convenience, Semble may also support:

```nix
inputs.semble.lib.operatorSshArtifacts {
  root = ./.;
}
```

The `root` form is a convenience wrapper. The preferred integration path is to
load repo metadata once at the flake boundary and pass normalized data down to
modules.

Semble should also expose a pure repo loader:

```nix
inputs.semble.lib.loadRepo {
  root = ./.;
}
```

`loadRepo` reads `${root}/semble.toml`, resolves Semble-managed paths such as
`hosts_dir` and `ssh_host_keys_dir`, imports host manifests, and returns a
normalized metadata model.

`operatorSshArtifacts` consumes that model and returns structured artifacts plus
rendered text:

```nix
{
  aliases = [ ... ];
  knownHosts = [ ... ];
  sshConfigText = ''...'';
  knownHostsText = ''...'';
}
```

The helper does not produce mutable files. Nix module evaluation can turn the
returned text into store-backed `/etc` entries through normal declarative config.

High-level flow:

1. Read `semble.toml` through `loadRepo`.
2. Resolve `hosts_dir` and `ssh_host_keys_dir` from Semble config.
3. Import host manifests from the configured hosts directory.
4. Select hosts where `operator.role = "server"`.
5. Resolve each server target from `operator.hostName` or `hostName`.
6. Expand alias templates and merge explicit alias overrides.
7. Read each server public SSH host key from the configured key directory.
8. Render OpenSSH config text.
9. Render plain `known_hosts` text.
10. Return structured data and rendered text.

This is a pure data transformation pipeline over Semble repo metadata. It keeps
Semble's operator SSH feature logic in Semble while leaving system installation
policy to the consumer host module.

## Flake Integration

The repo root should be supplied at the flake/Semble integration layer, not from
inside the SSH aliases module.

Recommended shape:

```nix
let
  sembleRepo = inputs.semble.lib.loadRepo {
    root = ./.;
  };

  semble = {
    repo = sembleRepo;
    operatorSshArtifacts = inputs.semble.lib.operatorSshArtifacts {
      repo = sembleRepo;
    };
  };
in
inputs.semble.lib.mkFlake {
  root = ./.;
  specialArgs = {
    inherit semble;
  };
}
```

The namespace should be a single Semble-provided module argument, such as
`semble`, instead of loose top-level arguments. This avoids name collisions and
keeps future Semble-provided repo metadata and derived artifacts grouped.

`operatorSshArtifacts` is repo-wide data. It is safe to derive regardless of the
currently evaluated host because the artifact content only includes hosts with
`operator.role = "server"`. Per-host installation is still gated by the consumer
module's own enable option.

## Semble CLI Behavior

### Explicit command

Semble exposes:

```bash
semble host ssh generate
```

Behavior:

1. Load normalized repo metadata using the same semantics as `loadRepo`.
2. Select all `operator.role = "server"` hosts.
3. Render the final `semble-servers.conf` text.
4. Render the plain SSH known-hosts artifact.
5. Print or write the artifacts for inspection/debugging.

The CLI command is useful for humans and tooling that want an inspectable export,
but it is not the primary Nix integration path. Client host builds should not
depend on untracked generated cache files.

### Automatic generation

For hosts with:

```nix
operator.role = "client"
```

Semble may run the inspection/export command before:

- `semble host build`
- `semble host switch`

This is optional workflow assistance for humans. The declarative Nix integration
does not require generated cache files because host modules consume rendered text
from the Semble Nix library.

For non-client hosts, Semble does not auto-run the inspection/export command.

## Consumer Repo Responsibility

The consumer repo should no longer own alias or known-host generation rules.

Its responsibility becomes:

- receive `semble.operatorSshArtifacts` from flake-level Semble integration
- install `semble.operatorSshArtifacts.sshConfigText` into
  `/etc/ssh/ssh_config.d/semble-servers.conf`
- install `semble.operatorSshArtifacts.knownHostsText` into the SSH known-hosts
  path used by the client host
- include that file in SSH client config
- keep any unrelated client-local SSH defaults, such as the `Host *` block

The consumer repo should not:

- crawl `hosts/`
- import every host manifest to generate aliases
- render alias text itself
- define operator-target known-host entries itself

## Why Keep Installation In Host Config

Semble should render the artifact text, but host config should still install it.

This keeps machine state declarative:

- Semble renders SSH config and known-hosts text
- the host config places it in `/etc/ssh/ssh_config.d/`
- SSH consumes it via normal include behavior

We do not want Semble to mutate `/etc/ssh/` directly on the live machine.

## Rensemble Migration

`rensemble` should be simplified to match this boundary.

Keep:

- `operator` metadata in host manifests
- flake-level Semble metadata/artifact wiring under a `semble` module argument
- client-side install/include wiring for rendered SSH artifacts
- client-local `Host *` SSH defaults

Remove:

- repo-local alias rendering logic
- repo-local known-hosts generation
- host-manifest crawling for SSH artifact generation
- any Nix-side generation that duplicates Semble behavior
- any dependency on untracked `.semble/cache/ssh` files for flake evaluation

## Verification Plan

The migration is successful when:

1. `inputs.semble.lib.loadRepo { root = ./.; }` reads `semble.toml` and host
   manifests from configured paths.
2. `inputs.semble.lib.operatorSshArtifacts { repo = semble.repo; }` returns
   `sshConfigText` and `knownHostsText`.
3. client hosts install `sshConfigText` into
   `/etc/ssh/ssh_config.d/semble-servers.conf`
4. client hosts consume `knownHostsText` through the configured known-hosts path
5. client SSH config includes that installed file
6. direct rebuilds do not require untracked generated files
7. rendered alias output matches the previous expected host aliases, including:
   - `<server-a>-admin`
   - `<server-a>-deploy`
   - `<builder-host>-admin`
   - `<builder-host>-deploy`
   - `<builder-host>-builder`
   - any explicit aliases like `bootstrap-host`
8. hosts with `aliasTemplates = [ ]` generate only their
   explicit aliases and do not get default `-admin` / `-deploy` aliases
9. generated known-hosts content covers the operator targets previously
   hardcoded in `rensemble`
