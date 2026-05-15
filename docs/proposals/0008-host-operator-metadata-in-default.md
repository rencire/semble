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
- [Generated File Path](#generated-file-path)
- [Missing File Behavior](#missing-file-behavior)
- [Semble CLI Behavior](#semble-cli-behavior)
- [Consumer Repo Responsibility](#consumer-repo-responsibility)
- [Why Not `/tmp`](#why-not-tmp)
- [Why Keep Installation In Host Config](#why-keep-installation-in-host-config)
- [Rensemble Migration](#rensemble-migration)
- [Verification Plan](#verification-plan)

## Summary

Move operator SSH alias generation out of `rensemble`'s Nix modules and into
Semble.

Host manifests in `default.nix` remain the source of truth for operator
metadata. Semble reads that metadata, generates the final
`semble-servers.conf` file and SSH known-hosts artifact, and writes them to
repo-local generated paths. Client hosts then install and include those
generated files as part of their SSH configuration.

This proposal supersedes the earlier draft direction where the consumer repo's
Nix module rendered aliases directly from host metadata.

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
promoted into the host manifest, Semble can read it directly, generate the SSH
config artifact once, and hand the finished result to consumer hosts.


## Proposed Change

The proposed design follows directly from that boundary:

- host manifests describe metadata
- Semble resolves and renders aliases
- consumer hosts only install and include the generated file

Add an `operator` field to `default.nix` and treat it as the source of truth
for operator SSH metadata.

Semble will read that metadata and generate the final SSH config text itself.
The generated file is named `semble-servers.conf` and lives at:

```text
.semble/cache/ssh/semble-servers.conf
```

Semble also generates a plain SSH known-hosts artifact at:

```text
.semble/cache/ssh/known_hosts
```

Client hosts consume both generated files.

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

- `server`: this host contributes alias metadata to the generated config
- `client`: this host consumes the generated config

Semble behavior:

- `operator.role = "server"`: include this host in alias generation
- `operator.role = "client"`: regenerate aliases before `host build` and
  `host switch`

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

## Generated File Path

Semble writes the final file to:

```text
.semble/cache/ssh/semble-servers.conf
```

This path is intentionally:

- repo-local
- generated
- outside `/tmp`
- outside the Nix store
- namespaced under `.semble`

This keeps the file durable across rebuild attempts while making it clear that
it is generated Semble-owned state.

## Missing File Behavior

Consumer hosts should fail fast if the generated file is missing.

We explicitly do not want a no-op fallback.

If a client host rebuild runs without a generated file, evaluation should fail
with a clear error telling the operator to regenerate the file with Semble.

Reason:

- a missing file means the workflow is broken
- silently succeeding would hide missing aliases
- fail-fast keeps the state model honest

## Semble CLI Behavior

### Explicit command

Semble exposes:

```bash
semble host ssh generate
```

Behavior:

1. Evaluate host manifests from `default.nix`
2. Select all `operator.role = "server"` hosts
3. Render the final `semble-servers.conf` text
4. Render the plain SSH known-hosts artifact
5. Write `.semble/cache/ssh/semble-servers.conf`
6. Write `.semble/cache/ssh/known_hosts`

### Automatic generation

For hosts with:

```nix
operator.role = "client"
```

Semble should automatically run alias generation before:

- `semble host build`
- `semble host switch`

This regeneration should happen every time, not only when the file is stale or
missing.

For non-client hosts, Semble does not auto-run alias generation.

## Consumer Repo Responsibility

The consumer repo should no longer generate alias text or known-host data.

Its responsibility becomes:

- install `.semble/cache/ssh/semble-servers.conf` into
  `/etc/ssh/ssh_config.d/semble-servers.conf`
- install `.semble/cache/ssh/known_hosts` into the SSH known-hosts path used by
  the client host
- include that file in SSH client config
- keep any unrelated client-local SSH defaults, such as the `Host *` block

The consumer repo should not:

- crawl `hosts/`
- import every host manifest to generate aliases
- render alias text itself
- define operator-target known-host entries itself

## Why Not `/tmp`

`/tmp` is not the right place for the generated file because it is ephemeral and
not reliable across rebuild workflows.

A repo-local cache path is better because it is:

- stable enough for rebuilds
- easy to inspect
- easy to clean
- easy to ignore in git
- clearly associated with Semble

## Why Keep Installation In Host Config

Semble should generate the artifact, but host config should still install it.

This keeps machine state declarative:

- Semble generates the file
- the host config places it in `/etc/ssh/ssh_config.d/`
- SSH consumes it via normal include behavior

We do not want Semble to mutate `/etc/ssh/` directly on the live machine.

## Rensemble Migration

`rensemble` should be simplified to match this boundary.

Keep:

- `operator` metadata in host manifests
- client-side install/include wiring for generated SSH artifacts
- client-local `Host *` SSH defaults

Remove:

- repo-local alias rendering logic
- repo-local known-hosts generation
- host-manifest crawling for SSH artifact generation
- any Nix-side generation that duplicates Semble behavior

## Verification Plan

The migration is successful when:

1. `semble host ssh generate` writes
   `.semble/cache/ssh/semble-servers.conf`
2. `semble host ssh generate` writes `.semble/cache/ssh/known_hosts`
3. client hosts with `operator.role = "client"` install that file into
   `/etc/ssh/ssh_config.d/semble-servers.conf`
4. client hosts consume the generated `known_hosts` artifact
5. client SSH config includes that installed file
6. direct rebuilds fail clearly if the file is missing
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
