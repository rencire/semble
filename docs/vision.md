# Semble Vision

## Summary

Semble is a Nix-based system composition tool for defining and assembling
managed hosts from reusable building blocks.

It provides an opinionated way to structure host configuration so systems stay
easier to understand, reuse, and evolve as they grow.

It does that by separating concerns clearly:

- Hosts that express identity and local intent
- Images that express packaging of a host into a boot artifact
- Profiles that express broad baseline composition
- Presets that express reusable bundles and conventional defaults
- Modules that express option schema, behavior, and upstream integrations
- Input module escape hatches for direct upstream composition when needed

Semble is meant to keep systems readable as they grow from one machine to many.

Semble is built around strong file and directory conventions, with explicit
escape hatches for host-local overrides when needed.

## Problem

NixOS is powerful, but configuration often becomes difficult to scale once a
codebase grows beyond a few hosts. Common failure modes include:

- Host files that accumulate too much logic
- Reusable concerns that get copied instead of composed
- Upstream flake integration details that leak into many places

Semble exists to impose a stronger structure on that problem.

## Core Idea

Semble models a system as a small set of composable layers:

- A host that names a specific managed system
- An image that packages a host into a boot artifact
- A profile that describes a broad baseline
- A preset that selects modules and provides conventional values
- A module that defines options, behavior, and required upstream imports
- An optional input-module escape hatch for direct upstream usage

This yields a clear composition flow:

`host -> profiles -> presets -> modules -> inputs`

with `image -> host` for artifact packaging.

with `host -> inputModules` available only as an explicit escape hatch.

The goal is not to hide Nix, but to make system structure obvious and
enforceable.

## Design Principles

### Small Host Definitions

Host files should stay short and identity-focused. They should answer questions
like:

- What system this is
- What role it has
- Which opinionated presets it opts into

They should not be the main place where reusable behavior is invented.

### Clear Ownership

Each layer should own a different kind of decision:

- Hosts owning identity, host-local overrides, and explicit composition choices
- Profiles owning broad baseline composition
- Presets owning reusable bundles and conventional defaults
- Modules owning schema, behavior, and upstream integration knowledge
- Input modules serving as temporary or host-specific direct upstream escape hatches

This keeps responsibilities clear and reduces overlap.

### Static Structure, Dynamic Behavior

Semble should keep the import graph predictable while allowing behavior to vary
through options and values. Composition should be explicit.

### Facade Over Upstream Module Systems

Semble defines its own module-facing interface as a facade over upstream module
systems. In v1 that facade is centered on NixOS modules; over time it should be
able to sit on top of other module ecosystems such as nix-darwin and Home
Manager without forcing hosts to absorb those integration details directly.

Semble should not reimplement the underlying module evaluators. It should
provide a more uniform composition surface while relying on those systems for
evaluation and behavior.

### Strictness Over Magic

Semble should prefer hard errors over silent ambiguity. Unknown keys, invalid
references, duplicate ownership selections, and repeated values inside one
selection list should fail early. Repeated `modules` and `inputModules` across
composition sources are dependency declarations instead; Semble deduplicates
them deterministically so presets can declare their own requirements directly.

## What Semble Is

Semble is:

- A composition model for managed hosts
- A facade module system built on top of upstream module systems
- A way to organize reusable NixOS concerns cleanly
- A boundary between reusable abstractions and host-local intent
- A stricter interface for assembling systems from input modules, modules, presets, and
  profiles
- An opinionated structure for organizing host configuration

## What Semble Is Not

Semble is not:

- A replacement for NixOS modules
- An attempt to reimplement the underlying NixOS, nix-darwin, or Home Manager
  module evaluators
- A general-purpose orchestration platform
- A dynamic runtime scheduler
- A tool that removes the need to understand Nix
- A framework for hiding every upstream edge case behind abstraction

Its job is narrower: provide a disciplined structure for host configuration.

## Intended Outcome

If Semble succeeds, a user should be able to:

- Understand why a host has a given behavior
- Find where a concern belongs without guessing
- Reuse configuration intentionally instead of copying it
- Add new hosts without growing incidental complexity
- Integrate upstream modules without scattering that knowledge across the tree

The system should feel composable, inspectable, and unsurprising.
