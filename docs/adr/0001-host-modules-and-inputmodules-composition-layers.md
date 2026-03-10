# ADR-0001: Add host `modules` and `inputModules` composition layers

## Status

Accepted

## Date

2026-03-09

## Context

The original layering discussion showed that forcing every capability to flow
through a preset makes presets do too much and pushes consumers toward trivial
one-module presets. At the same time, consumers still need a clean way to use
raw upstream modules when no meaningful Semble abstraction exists yet.

## Decision

- Hosts may compose local Semble modules directly through `modules`.
- Hosts may compose raw upstream modules directly through `inputModules`.
- Presets remain the reusable bundling/defaults layer.
- Profiles remain the broad baseline layer composed from presets.
- `inputModules` remain the explicit raw-upstream escape hatch when no local
  Semble module abstraction exists yet.

## Consequences

- Hosts can express intent directly without inventing trivial presets.
- The API keeps a clear distinction between local Semble abstractions and raw
  upstream imports.
- Consumers can start at `modules` or `inputModules` and promote upward only
  when repetition appears.

## Alternatives Considered

- Require every capability to travel through a preset.
- Require every upstream import to be wrapped in a local Semble module.

## Notes

This ADR establishes the basic entry-layer split that later composition
refinements build on.
