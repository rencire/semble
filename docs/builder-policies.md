# Builder Policies

This document sketches a minimal Semble design for command-scoped builder
selection.

It is a design note, not part of the current stable API contract.

## Table Of Contents

- [Problem](#problem)
- [Goal](#goal)
- [Non-Goals](#non-goals)
- [Minimal CLI](#minimal-cli)
- [Minimal `semble.toml` Shape](#minimal-sembletoml-shape)
- [Policy Shape](#policy-shape)
- [Field Meaning](#field-meaning)
- [Why This Belongs In `semble.toml`](#why-this-belongs-in-sembletoml)
- [Relationship To Host Nix Config](#relationship-to-host-nix-config)
- [Validation Rules](#validation-rules)
- [Operational Notes](#operational-notes)
- [Deferred Questions](#deferred-questions)

## Problem

Some deployments need a specific build machine for operational reasons that Nix
does not model directly. A common case is:

- more than one builder is technically compatible with the requested `system`
- one of those builders is operationally unsuitable for a given workload
- the operator wants a short Semble flag rather than raw Nix builder syntax

Examples:

- a Linux VM can build many `x86_64-linux` derivations, but a physical Linux
  host is required for one problematic package
- the same-machine Nix daemon should evaluate and orchestrate, but not
  participate as a fallback builder for that invocation

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
semble host switch my-host --target-host my-host-deploy --builder-policy buildbox
```

The same flag should be accepted by:

- `semble host build`
- `semble host switch`
- `semble host provision`

Semble resolves the named policy before delegating to the underlying build or
deploy command.

`--builder-policy` constrains builder selection for the delegated build. It does
not change the deployment target selected by flags such as `--target-host`.

## Minimal `semble.toml` Shape

```toml
[[builder_policies]]
name = "buildbox-builder-preferred"

[builder_policies.local]
max_jobs = 1

[[builder_policies.builders]]
host = "builder@buildbox.example.net"
system = "x86_64-linux"
ssh_key = "/etc/nix/ssh/buildbox_builder"
max_jobs = 6
speed_factor = 1
supported_features = ["benchmark", "big-parallel", "kvm", "nixos-test"]
```

## Policy Shape

The current policy shape has three roles:

- `name`: CLI-facing policy identifier
- `local`: same-machine Nix daemon scheduling for this invocation
- `builders`: delegated Nix builder machine specs

A strict builder-only policy sets or omits `local.max_jobs = 0`. A
builder-preferred policy sets `local.max_jobs = 1` or another positive value.

Operationally, Semble translates the selected policy into standard Nix settings:

- `builders = <serialized delegated builders>`
- `max-jobs = <local.max_jobs>`

For `nh` build and switch commands, Semble passes those values as command-line
flags. For `nixos-anywhere` provision commands, Semble merges them into
`NIX_CONFIG` so the internal `nix build` commands see the same policy.

## Field Meaning

- `name`: CLI-facing policy identifier used by `--builder-policy`
- `local.max_jobs`: same-machine Nix daemon jobs for this invocation; defaults
  to `0` when omitted
- `builders[].host`: SSH destination Semble should use when serializing the
  delegated builder
- `builders[].system`: Nix platform this builder should advertise
- `builders[].ssh_key`: optional private key path to serialize into the builder
  entry
- `builders[].max_jobs`: serialized delegated builder job count
- `builders[].speed_factor`: serialized builder weighting hint
- `builders[].supported_features`: serialized Nix builder features

`builders[].host` should already be a valid SSH destination string, suitably
usable by the actual Nix remote-builder SSH path.

In practice, builder policies are more robust when `host` is a real
`user@hostname` destination instead of a shell-only SSH alias, because Nix's
remote-builder transport may not resolve aliases from the same config context as
an interactive user shell.

`builders[].ssh_key` is optional. When present, Semble serializes it into Nix's
builder entry so the delegated command does not have to rely on ambient
`ssh-agent` state. This is especially useful on macOS multi-user Nix setups
where delegated builder SSH runs under the daemon/build-user context rather than
the interactive user shell.

## Why This Belongs In `semble.toml`

Builder policy is execution-time orchestration metadata:

- it affects how Semble invokes Nix
- it does not become part of the built target system
- it should be resolved before the delegated command starts
- the intended abstraction is broader than a single host, but the current
  implementation only narrows to one host at a time

That makes it closer to existing `semble.toml` concerns like repo paths and
execution-time command metadata than to host-local NixOS configuration.

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
- a builder policy omits any required field or has no `builders[]` entries
- a selected delegated builder cannot be used for the delegated build

Semble should treat duplicate `builder_policies.name` values as invalid config.

Semble should fail rather than silently falling back to same-machine builds or
ambient machine-wide builders when a builder policy is selected.

## Operational Notes

Builder policies only solve builder selection. Each delegated builder account
still needs:

- an SSH-reachable account on the target machine
- a non-interactive private key readable by the local Nix builder context
- any required `trusted-users` or remote-build permissions on the target host

For automated delegated builds, a dedicated builder key without a passphrase is
usually more reliable than reusing a human deploy key.

## Deferred Questions

These are intentionally out of scope for the first version:

- deriving builder policy from host definitions instead of explicit TOML fields
- adding a separate `--builder` shorthand
- Semble-managed fallback chains or dynamic builder discovery
