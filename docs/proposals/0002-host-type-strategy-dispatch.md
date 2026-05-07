# Proposal: Dispatch host lifecycle by declared host type

Status: `accepted`

## Summary

Introduce a `type` field in `hosts/<name>/default.nix` so Semble can dispatch
host lifecycle behavior from the host definition itself.

The first two supported types would be:

- `physical` for normal machine provisioning and switching
- `microvm` for guest-image provisioning and parent-host-backed activation

This would let Semble collapse the separate `microvm provision` command into
the existing host lifecycle surface while keeping the workflow type-aware.
The current MicroVM command path is a migration bridge, not the long-term
public API.

## Problem

Semble currently has two overlapping concepts:

- a generic host lifecycle surface under `host`
- a separate `microvm provision` path for MicroVM guest provisioning

That split works, but it duplicates lifecycle semantics across command
families. The actual distinction is not "host vs microvm command"; it is
whether the host definition is a physical machine or a MicroVM-backed guest.

As a result:

- command shape is carrying behavior that belongs in data
- provisioning behavior cannot be selected from the host definition alone
- MicroVM-specific lifecycle logic is exposed as a separate top-level command

## Proposed Change

Add a `type` field to the host definition at `hosts/<name>/default.nix`.

Semble would read that field and use it to choose the lifecycle strategy for
the host.

Initial supported values:

1. `physical`
2. `microvm`

Behavioral consequences:

- `host create` selects the correct template and starter layout for the type.
- `host provision` dispatches to the physical-host or MicroVM provisioning
  flow based on the declared type.
- `host switch` stays type-aware where activation differs.
- The dedicated `microvm provision` command remains only as a transitional
  implementation path while the host-type strategy is being introduced, then
  is removed once the host path covers the same behavior.
- MicroVM hosts should carry a `provisionTarget` field naming the SSH endpoint
  used for provisioning the guest image on the parent machine.
- MicroVM hosts should emit a clear reminder that the parent host still needs
  `microvm.host` / `virtualization.microvm-host` wired in and activated
  separately before the guest can run.
- MicroVM provisioning should alert the user that the parent host still needs
  to be switched with `host switch` before the guest becomes runnable.

## Command Model

Preferred shape:

- `semble host create <host>`
- `semble host provision <host>`
- `semble host switch <host>`

The host definition determines whether those commands operate on a physical
machine or a MicroVM guest.

Rejected shape:

- `semble host provision --microvm`
- `semble microvm provision <guest>`

Those shapes keep behavior in the command name instead of in the host
definition, which makes the lifecycle model harder to reason about and harder
to extend.

## Why This Is Better

- The host definition becomes the source of truth for lifecycle strategy.
- Semble keeps one host-oriented CLI surface instead of parallel physical and
  MicroVM command families.
- Template selection, provisioning, and activation can all key off the same
  declared type.
- The model matches the fact that both physical hosts and MicroVM guests are
  managed systems, even if their provisioning mechanics differ.

## Implementation Notes

- `hosts/<name>/default.nix` would need a new required or strongly-validated
  `type` field.
- `microvm` hosts would need a `provisionTarget` field naming the SSH
  destination Semble should use during provisioning.
- Host creation would need a type-aware template selection mechanism.
- MicroVM host creation should print a follow-up note about enabling the
  parent host's MicroVM runtime module before provisioning the guest.
- MicroVM provisioning should print a follow-up note that the parent host must
  still be switched after the guest image is provisioned.
- Provisioning and activation dispatch would need to route through a strategy
  layer keyed by host type.
- MicroVM-specific provisioning behavior would move behind the host lifecycle
  path rather than living in a separate top-level command.

## Migration Plan

1. Add the host `type` field and make it mandatory or defaulted with explicit
   validation.
2. Teach `host create` to select templates by type.
3. Teach `host provision` and related lifecycle commands to dispatch by type.
4. Migrate the current `microvm provision` implementation behind the host
   lifecycle path as the MicroVM strategy backend.
5. Remove the standalone `microvm provision` command once parity is complete.

## Validation Plan

Before the full host-type refactor lands, validate the current MicroVM
provisioning path against the sibling `rensemble` repo:

1. Update the local Semble input in `rensemble` with `nix flake update semble`.
2. Run `nix develop . -c semble microvm provision test-mvm --parent thor-admin --no-encrypt --builder-policy thor`.
3. Confirm the parent directory and backing image both end up owned by
   `microvm:kvm`.
4. Repeat the same provisioning flow with encryption enabled and a root unlock
   key under `secrets/luks_root_keys/test-mvm`.

## Validation Results

The current implementation was exercised end to end in the sibling
`rensemble` repo after updating its local Semble input:

1. `nix flake update semble`
2. `nix develop . -c semble host provision test-mvm --no-encrypt --builder-policy thor`
3. `nix develop . -c semble host provision test-mvm --key-file secrets/luks_root_keys/test-mvm-root.key --builder-policy thor --force-reformat`

Those runs confirmed:

- `host provision` dispatches by host `type`
- plain and encrypted MicroVM provisioning both complete successfully
- the parent directory and backing image are owned by `microvm:kvm`
- encrypted provisioning uses the expected root unlock key path under
  `secrets/luks_root_keys/`

## Open Questions

- Should `type` be required for every host immediately, or should Semble infer
  `physical` as a temporary default?
- What file layout should type-specific templates use?
- Should `provisionTarget` be the only transport field for MicroVM provisioning,
  or do we eventually want Semble to derive SSH destinations from host aliases?
- Do `delete` and `keys` remain type-agnostic, or do we eventually need
  type-specific cleanup behavior?

## Notes

This proposal is broader than the current MicroVM provisioning migration.
It is about moving lifecycle selection into host data so the CLI can stay
single-surfaced while still supporting different host strategies.
