# Semble Architecture

This document records the higher-level decisions behind
the consumer-facing API in `docs/api.md`.

It explains why Semble is structured this way and what each layer owns. It does
not define file formats, field names, or precedence rules.

Shared vocabulary is defined in `docs/terminology.md`.

## Architectural Goals

- Keep host files small and intent-focused.
- Keep upstream dependency knowledge in modules, not in hosts.
- Keep preset logic as reusable bundles of module selection plus conventional values.
- Keep profile logic as broad host-baseline composition.
- Favor strong conventions over broad public constructors.

## Current Architectural Decisions

1. Consumer authoring is convention-based rather than constructor-based.
2. Hosts compose `profiles`, `presets`, local `modules`, and raw `inputModules`.
3. Images package resolved hosts into boot artifacts through a separate root-level `images/` convention.
3. Modules own schema, behavior, and upstream integration knowledge.
4. Presets own reusable bundles of module selection and default values.
5. Profiles own broad baseline composition of presets.
6. `inputModules` are an explicit escape hatch for direct upstream usage, not the preferred steady-state abstraction.
6. Import structure stays static while behavior varies through options and
   values.

## Notes

These are not full ADRs yet. They may later be split into separate ADRs.
