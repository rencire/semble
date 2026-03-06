# Semble Terminology

This document defines shared vocabulary used across Semble's docs.

## Host

A specific managed system.

## Host Definition

The file at `hosts/<name>/default.nix` that defines host identity and host
composition selections.

## Profile

A role-level composition unit. A profile is composed of presets.

## Preset

A reusable bundle of module selections and values. A preset selects modules and
assigns values to existing options.

## Module

A Semble unit that defines option schema, behavior, and dependencies to upstream modules.

## Host Configuration File

A host-local module, usually `hosts/<name>/configuration.nix`, applied after
preset values and default host-derived values.


## Value Application

The precedence process that applies defaults, preset values, host-derived
values, and host-local overrides.
