# Releasing

## Policy

Semble uses Conventional Commits and git tags to drive releases.

- Release boundaries are tags.
- Version bumps are derived from merged commits since the previous tag.
- `feat` commits bump the minor version.
- `fix` commits bump the patch version.
- Breaking changes marked with `!` or `BREAKING CHANGE:` bump the major version.
- `docs`, `test`, `chore`, and `refactor` commits do not affect the release
  version.
- No prereleases by default.

Main should stay buildable, but it may include integration work when that work
is intentionally part of the release train. Breaking changes are allowed when
they are explicitly marked and intentionally released.

The release process should keep versioned metadata in sync for whatever package
or language ecosystem the repo uses.

Examples:

- Rust crates: `Cargo.toml`, lockfiles, and any Nix package version pins
- JavaScript packages: `package.json`, lockfiles, and published package metadata
- Python packages: `pyproject.toml`, `setup.cfg`, or wheel/sdist metadata
- Go modules: `go.mod` and any release tags or generated module docs

When a release needs explicit version metadata, prefer a dedicated release
commit before the tag rather than bumping versions in every normal development
commit. That keeps day-to-day commits focused on code changes while still making
the released state explicit.

```mermaid
flowchart LR
  A[Normal commits] --> B[Release commit\n(version metadata)]
  B --> C[Tag the release]
  C --> D[Published release]
```

## Examples

- `feat(host): add disk key discovery` -> minor release
- `fix(host): handle missing key file` -> patch release
- `feat(host)!: change provision key discovery` -> major release
- `docs(releasing): clarify version policy` -> no release bump
