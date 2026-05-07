# Proposal: Auto-detect provision encryption keys

Status: `proposed`

## Table of Contents

- [Summary](#summary)
- [Problem](#problem)
- [Proposed Behavior](#proposed-behavior)
- [Discovery Rules](#discovery-rules)
- [Override Behavior](#override-behavior)
- [Examples](#examples)
- [Open Questions](#open-questions)

## Summary

Semble should eventually be able to discover the right encryption-key input for
`host provision` automatically when the repo configuration makes that possible.

The goal is to remove manual flag passing for the common encrypted-provisioning
flows while keeping explicit overrides available.

## Problem

Today, encrypted provisioning requires users to remember which flag and path
shape applies to which workflow:

- MicroVM provisioning uses Semble's `--key-file`
- physical-host provisioning forwards `--disk-encryption-keys` to
  `nixos-anywhere`

That split is easy to forget, and it makes the command line noisier than it
needs to be when the repository already knows the right key location.

## Proposed Behavior

When `host provision` is invoked for an encrypted host, Semble should try to
discover the matching encryption key from repository configuration and inject
the correct mechanism automatically.

- For MicroVM hosts, Semble would auto-fill `--key-file`
- For physical hosts, Semble would auto-fill the appropriate
  `--disk-encryption-keys` pair

If discovery fails or is ambiguous, Semble should stop with a clear error and
tell the user which explicit flag to pass.

## Discovery Rules

The exact configuration namespace is intentionally unresolved in this proposal.
The main requirement is that Semble can deterministically resolve a key source
from host metadata.

Possible shapes include:

- a Semble-owned provisioning namespace
- a host-local config field alongside the host definition
- a conventional repo path derived from host name and `disk_keys_dir`
- a NixOS-evaluated host field that Semble reads from the configuration

The discovery mechanism should be:

- deterministic
- explicit enough to avoid surprises
- able to fail fast when the repo lacks the required metadata

One important constraint is avoiding duplicated key paths. If the same secret
location has to be written in both Semble metadata and NixOS module config, the
design should either make one side authoritative or generate the other from it.

## Override Behavior

Explicit user input should continue to work.

- If the user passes `--key-file`, Semble should use it for MicroVM flows
- If the user passes passthrough `--disk-encryption-keys`, Semble should not
  fight that choice for physical-host flows
- If both discovered values and explicit overrides exist, explicit input should
  win or Semble should surface a conflict, depending on the final design

## Examples

MicroVM example:

```bash
semble host provision my-vm
```

Physical-host example:

```bash
semble host provision thor --target-host genesis-nixos
```

In both cases, the intended behavior is that Semble discovers the encryption
key configuration from the repo and fills in the right underlying install-time
flag.

## Open Questions

- Should discovery live in a new Semble config namespace, or in per-host config
  beside the host definition?
- Should the key path be authored once in Semble and propagated into NixOS, or
  authored once in NixOS and evaluated by Semble?
- Should explicit flags override discovery silently, or should Semble reject
  conflicting inputs?
- Should the same convention cover both MicroVM and physical-host provisioning,
  or should they be independently configurable?
- What should the failure message look like when a host is marked encrypted but
  no key path can be found?
