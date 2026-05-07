# Releasing

## Policy

Semble uses Conventional Commits and git tags to drive releases.

- Release boundaries are tags.
- Version bumps are derived from merged commits since the previous tag.
- `feat` commits bump the minor version.
- `fix` commits bump the patch version.
- Breaking changes marked with `!` or `BREAKING CHANGE:` bump the major version.
- `docs`, `test`, `chore`, and `refactor` commits do not affect the release version.
- No prereleases by default.

Main should stay buildable, but it may include integration work when that work is intentionally part of the release train. Breaking changes are allowed when they are explicitly marked and intentionally released.

## Version Sync

The release process must keep version declarations in sync across the repo, including:

- `Cargo.toml`
- `nix/packages/semble.nix`

## Examples

- `feat(host): add disk key discovery` -> minor release
- `fix(host): handle missing key file` -> patch release
- `feat(host)!: change provision key discovery` -> major release
- `docs(releasing): clarify version policy` -> no release bump
