# Proposal: Review host lifecycle command vocabulary

Status: `draft`

## Table Of Contents

- [Summary](#summary)
- [Problem](#problem)
- [Current Vocabulary](#current-vocabulary)
- [Vocabulary Goals](#vocabulary-goals)
- [Option A: Keep NixOS Verbs](#option-a-keep-nixos-verbs)
- [Option B: Use Deployment-Stage Verbs](#option-b-use-deployment-stage-verbs)
- [Option C: Use Explicit Activation Modes](#option-c-use-explicit-activation-modes)
- [Recommendation](#recommendation)
- [Migration Concerns](#migration-concerns)
- [Open Questions](#open-questions)

## Summary

Evaluate whether Semble should keep the NixOS-inspired host lifecycle verbs
`build`, `boot`, and `switch`, or move toward vocabulary that better describes
Semble's product model across physical hosts, MicroVM guests, images, and future
deployment flows.

This proposal is intentionally separate from the concrete `host boot` behavior
proposal. The immediate safety fix should not be blocked on naming strategy.

## Problem

The NixOS verbs are familiar to NixOS operators, but they are not self-evident to
new Semble users:

- `build` means compile and realize a system closure, not build an image or
  provision a host
- `switch` means deploy and live-activate, not merely select a host or change a
  local context
- `boot` means install as the next boot generation, not reboot the machine

As Semble grows beyond one-to-one wrappers around NixOS commands, command names
should communicate Semble's behavior clearly without losing the affordances that
experienced NixOS users expect.

## Current Vocabulary

Current and proposed host operations are:

| Command | Current Meaning |
|---------|-----------------|
| `host build` | Build the host closure without deploying it. |
| `host boot` | Proposed: deploy as next boot generation without live activation. |
| `host switch` | Deploy and activate immediately. |
| `host provision` | Install a new machine or guest, including disk prep and installation. |

This is close to `nixos-rebuild` vocabulary for NixOS hosts.

## Vocabulary Goals

The command vocabulary should:

- make dangerous operations obvious before execution
- distinguish build, deploy, activation, reboot, and provisioning phases
- remain concise enough for frequent operator use
- preserve NixOS familiarity where it reduces mistakes
- scale to physical machines, MicroVM guests, image artifacts, and future host
  types
- avoid verbs whose plain-English meaning conflicts with Semble behavior

## Option A: Keep NixOS Verbs

Keep:

```bash
semble host build <host>
semble host boot <host> --target-host <target>
semble host switch <host> --target-host <target>
```

Advantages:

- lowest surprise for NixOS users
- maps cleanly to `nixos-rebuild build`, `boot`, and `switch`
- minimal CLI churn
- easy to implement incrementally

Disadvantages:

- `boot` can sound like it reboots the machine
- `switch` is jargon outside NixOS
- Semble remains framed as a wrapper rather than a higher-level host operations
  tool

## Option B: Use Deployment-Stage Verbs

Use verbs that describe Semble's operational stage directly:

```bash
semble host build <host>
semble host stage <host> --target-host <target>
semble host activate <host> --target-host <target>
```

In this model:

- `build` compiles without touching the target
- `stage` deploys the new generation for next boot without live activation
- `activate` deploys and activates immediately

Advantages:

- clearer to users who do not already know NixOS vocabulary
- `stage` avoids implying an immediate reboot
- `activate` says the dangerous part directly

Disadvantages:

- diverges from `nixos-rebuild` muscle memory
- `stage` may be ambiguous unless docs define it tightly
- migration would require aliases or deprecation policy

## Option C: Use Explicit Activation Modes

Use one deployment verb with an activation mode:

```bash
semble host deploy <host> --target-host <target> --mode boot
semble host deploy <host> --target-host <target> --mode switch
```

Possible mode names:

- `--mode next-boot`
- `--mode live`
- `--activate=false`
- `--activation next-boot`
- `--activation live`

Advantages:

- makes deploy the shared operation and activation the varying dimension
- reduces top-level verb count
- leaves room for future modes such as dry-run, test, reboot-after-install, or
  health-checked activation

Disadvantages:

- more typing for common operations
- flags can hide the most important safety distinction
- easier to accidentally choose the wrong mode if defaults are not obvious

## Recommendation

For the immediate `host boot` proposal, keep the NixOS-aligned verb because it is
the smallest safe extension and directly matches the desired behavior.

For long-term Semble vocabulary, the strongest alternative is Option B:
`build`, `stage`, and `activate`. It makes the live-activation boundary explicit
without forcing every deploy operation through a mode flag.

This proposal should decide whether Semble wants to remain NixOS-vocabulary-first
or introduce Semble-native aliases while preserving the NixOS verbs for expert
users.

## Migration Concerns

If Semble changes vocabulary, it should avoid breaking existing scripts abruptly.
Possible migration paths:

- keep `build`, `boot`, and `switch` as stable aliases
- add Semble-native aliases first, then document preferred verbs later
- emit hints for potentially confusing commands, such as reminding users that
  `host boot` does not reboot
- avoid removing NixOS-aligned verbs until Semble has a broader CLI stability
  policy

## Open Questions

- Should `host boot` display an explicit post-success message telling the user to
  reboot manually?
- Should `host switch` be renamed or aliased to `host activate`?
- Should `host boot` be renamed or aliased to `host stage`?
- Should Semble eventually have a single `host deploy` command with activation
  modes, or are separate verbs safer?
- Should command names differ between physical NixOS hosts and MicroVM guests, or
  should Semble force one lifecycle vocabulary across host types?
