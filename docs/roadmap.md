# Semble Roadmap

This document captures version-specific priorities that are expected to change
over time.

## Early v1 Priorities

The first version should optimize for:

- A small and coherent core API
- Explicit composition semantics
- Strong validation and hard errors
- Low conceptual overlap between abstraction layers

It should not try to solve every advanced workflow immediately. A smaller,
clearer model is more valuable than a larger, blurrier one.

Candidate future extension:

- command-scoped builder selection through `--builder-policy`; see
  [docs/builder-policies.md](./builder-policies.md)
