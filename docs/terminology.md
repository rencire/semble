# Semble Terminology

This document defines shared vocabulary used across Semble's docs.

## Host

A specific managed system.

## Host Definition

The file at `hosts/<name>/default.nix` that defines host identity and host
composition selections.

## Profile

A broad baseline composition unit. A profile is composed of presets.

## Preset

A reusable bundle of module selections and conventional values. A preset selects modules and
assigns values to existing options.

## Module

A Semble unit that defines option schema, behavior, and dependencies to upstream modules. Hosts should normally reference modules rather than upstream modules directly.

## Host Configuration File

A host-local module, usually `hosts/<name>/configuration.nix`, applied after
preset values and default host-derived values.


## Value Application

The precedence process that applies defaults, preset values, host-derived
values, and host-local overrides.

## Input Module

A raw upstream module reference such as `microvm.host`, resolved through the consumer flake's `inputs`. This is an explicit escape hatch for host composition when no local Semble wrapper module exists yet.

## Image

A bootable artifact definition that packages a resolved host configuration.

## Image Definition

The file at `images/<name>/default.nix` that defines how a host should be packaged as an artifact.
