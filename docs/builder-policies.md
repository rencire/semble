# Builder Policies

This document sketches a minimal Semble design for command-scoped builder
selection.

It is a design note, not part of the current stable API contract.

## Problem

Some deployments need a specific build machine for operational reasons that Nix
does not model directly. A common case is:

- more than one builder is technically compatible with the requested `system`
- one of those builders is operationally unsuitable for a given workload
- the operator wants a short Semble flag rather than raw `NIX_BUILDERS` syntax

Examples:

- a Linux VM can build many `x86_64-linux` derivations, but a physical Linux
  host is required for one problematic package
- the controller host should evaluate and orchestrate, but not participate as a
  fallback builder for that invocation

## Goal

Provide a small Semble-owned orchestration API that:

- selects a named builder policy for a single command invocation
- translates that policy into the appropriate Nix builder overrides
- keeps this execution-time policy in `semble.toml`, not in host system config

## Non-Goals

- model arbitrary multi-builder routing policy in the first version
- replace Nix's scheduler
- move persistent machine-wide Nix defaults out of host config

## Minimal CLI

```bash
semble host switch thor --target-host thor-deploy --builder-policy l380y
```

The same flag should be accepted by:

- `semble host build`
- `semble host switch`
- `semble host provision`

Semble resolves the named policy before delegating to the underlying build or
deploy command.

`--builder-policy` constrains builder selection for the delegated build. It
does not change the deployment target selected by flags such as `--target-host`.

## Minimal `semble.toml` Shape

```toml
[[builder_policies]]
name = "l380y"
host = "l380y-deploy"
system = "x86_64-linux"
max_jobs = 6
speed_factor = 1
supported_features = ["benchmark", "big-parallel"]
```

## First-Version Scope

The first version supports exactly one policy shape:

- the policy resolves to exactly one remote builder entry
- Semble serializes that builder into Nix's builder override syntax
- no other remote builders are injected for that invocation

This first version is always strict:

Operationally, the first version can implement this by setting both:

- `NIX_BUILDERS=<serialized selected builder>`
- `NIX_CONFIG=max-jobs = 0`

In the first version, Semble should emit exactly one strict builder entry for
the selected policy and should not expose additional builder-selection modes.

Semble should not expose looser modes until there is a concrete need for them.

## Field Meaning

- `name`: CLI-facing policy identifier used by `--builder-policy`
- `host`: SSH destination Semble should use when serializing the builder
- `system`: Nix platform this builder should advertise
- `max_jobs`: serialized builder job count
- `speed_factor`: serialized builder weighting hint
- `supported_features`: serialized Nix builder features

For the first version, `host` should already be a valid SSH destination string,
such as a generated Semble alias like `l380y-deploy`.

For the first version, Semble should assume the normal SSH-based Nix remote
builder transport and should not expose per-policy transport or authentication
fields.

## Why This Belongs In `semble.toml`

Builder policy is execution-time orchestration metadata:

- it affects how Semble invokes Nix
- it does not become part of the built target system
- it should be resolved before the delegated command starts

That makes it closer to existing `semble.toml` concerns like SSH alias
conventions than to host-local NixOS configuration.

## Relationship To Host Nix Config

Builder policy does not replace persistent Nix configuration.

Machine-local Nix config still owns:

- default `buildMachines`
- default `distributedBuilds`
- machine-wide cache and daemon settings

Builder policy is a command-scoped override layer on top of those defaults.

This allows a repo to:

- keep a normal default builder setup in host config
- use `--builder-policy <name>` only for exceptional deploy/build runs

## Validation Rules

Semble should hard-error when:

- `--builder-policy` names an unknown policy
- a builder policy omits any required field
- the selected builder cannot be used for the delegated build

Semble should treat duplicate `builder_policies.name` values as invalid config.

Semble should fail rather than silently falling back to local builds or other
configured builders when a builder policy is selected.

## Deferred Questions

These are intentionally out of scope for the first version:

- multiple builders in one policy
- policy kinds such as pools, fallback chains, or feature-routed policies
- deriving builder policy from host definitions instead of explicit TOML fields
- adding a separate `--builder` shorthand
