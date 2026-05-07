# Proposal: Select host creation templates by name

Status: `draft`

## Summary

Allow `host create` to choose a template by name while keeping a required default template in repo config.

The default behavior stays explicit:

- `semble host create <host>` uses the configured default template name
- `semble host create <host> --template <name>` uses a named template

## Problem

Semble currently assumes one host template layout for every created host.

That works while all hosts share one starter shape, but it becomes too rigid once repos want different scaffolds for different host families.

## Proposed Change

Add an optional `--template <name>` flag to `host create`.

Preferred shape:

- `semble host create <host>`
- `semble host create <host> --template <name>`

Behavior:

- If `--template` is omitted, Semble uses the configured default template name.
- If `--template` is provided, Semble resolves the named template relative to the configured host template root.
- Template names are repo-defined and arbitrary.
- Semble does not persist a template marker in the generated host definition.

## Configuration Model

`semble.toml` defines both the host template root and the default template name.

Required settings:

- `host_template_dir`: the template root directory
- `default_host_template`: the fallback template name

Resolution rules:

- default template: `host_template_dir/default_host_template`
- named template: `host_template_dir/<name>`

## Why This Is Better

- Keeps the default behavior unchanged for existing repos.
- Makes template choice explicit only when needed.
- Avoids adding host metadata that would need to stay in sync later.
- Leaves template naming fully under repo control.

## Migration Plan

1. Keep the current default template behavior intact.
2. Teach `host create` to accept `--template`.
3. Resolve the selected template directory from the repo config.
4. Add tests for default fallback and named template lookup.

## Validation Plan

Before merging, validate these cases:

1. `host create <host>` uses `default_host_template` when `--template` is omitted.
2. `host create <host> --template <name>` resolves the named template under `host_template_dir`.
3. `host create <host> --template <missing>` fails clearly before creating the host directory.

## Open Questions

- None for now.
