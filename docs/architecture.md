# Semble Architecture

This document records the higher-level decisions behind
the consumer-facing API in `docs/api.md`.

It explains why Semble is structured this way and what each layer owns. It does
not define file formats, field names, or precedence rules.

Shared vocabulary is defined in `docs/terminology.md`.

## Architectural Goals

- Keep host files small and intent-focused.
- Keep upstream dependency knowledge in modules, not in hosts.
- Keep preset logic as reusable value bundles.
- Keep profile logic as host-role composition.
- Favor strong conventions over broad public constructors.

## Current Architectural Decisions

1. Consumer authoring is convention-based rather than constructor-based.
2. Host composition is flat through `profiles` and `presets`.
3. Modules own schema, behavior, and upstream integration knowledge.
4. Presets own opinionated values and module selection.
5. Profiles own role-level composition of presets.
6. Import structure stays static while behavior varies through options and
   values.

## Notes

These are not full ADRs yet. They may later be split into separate ADRs.
