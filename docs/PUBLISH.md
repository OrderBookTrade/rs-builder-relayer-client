# Publishing to crates.io

Step-by-step guide to publish `polymarket-relayer` to crates.io.

## Prerequisites

- A [crates.io](https://crates.io) account (login with GitHub)
- An API token from [crates.io/settings/tokens](https://crates.io/settings/tokens)

## Step 1: Login to crates.io

```bash
cargo login <your-api-token>
```

This saves the token to `~/.cargo/credentials.toml`. You only need to do this once.

## Step 2: Verify Cargo.toml metadata

Make sure these fields are correct before publishing:

```toml
[package]
name = "polymarket-relayer"          # must be unique on crates.io
version = "0.1.0"                    # bump for each release
description = "..."                  # required
license = "MIT OR Apache-2.0"        # required
repository = "https://github.com/..." # update to your real repo URL
readme = "README.md"
keywords = ["polymarket", "relayer", "gasless", "ethereum", "polygon"]
categories = ["api-bindings"]
```

Things to check:
- [ ] `name` is not already taken — verify at `https://crates.io/crates/polymarket-relayer`
- [ ] `repository` points to your actual GitHub repo
- [ ] `authors` has your real name/email
- [ ] `version` follows semver (`0.1.0` for first release)
- [ ] `license` is set (required for publish)
- [ ] `description` is set (required for publish)

## Step 3: Dry-run publish

```bash
cargo publish --dry-run
```

This does everything except the actual upload:
- Checks metadata validity
- Builds the crate
- Verifies it compiles from the packaged source
- Shows what files will be included

Fix any errors before proceeding.

## Step 4: Check what gets packaged

```bash
cargo package --list
```

Review the file list. Make sure:
- No secrets (`.env`, credentials) are included
- No large binaries or test fixtures leak in
- `README.md` is included (shown on crates.io)

If you need to exclude files, add to `Cargo.toml`:

```toml
[package]
exclude = [".env", "target/", ".github/"]
```

## Step 5: Publish

```bash
cargo publish
```

That's it. Your crate is live at `https://crates.io/crates/polymarket-relayer`.

Users can now add it with:

```bash
cargo add polymarket-relayer
```

## After Publishing

### Verify it works

```bash
# In a fresh project
cargo init test-install && cd test-install
cargo add polymarket-relayer
cargo check
```

### Versioning for future releases

| Change type | Version bump | Example |
|---|---|---|
| Bug fix, no API change | Patch | `0.1.0` -> `0.1.1` |
| New feature, backwards-compatible | Minor | `0.1.1` -> `0.2.0` |
| Breaking API change | Major | `0.2.0` -> `1.0.0` |

To release a new version:

```bash
# 1. Update version in Cargo.toml
# 2. Commit
git add Cargo.toml && git commit -m "release: v0.1.1"
git tag v0.1.1

# 3. Publish
cargo publish

# 4. Push tag
git push origin v0.1.1
```

### Yanking a bad release

If you publish a broken version:

```bash
cargo yank --version 0.1.0          # prevent new downloads
cargo yank --version 0.1.0 --undo   # un-yank if it was a mistake
```

Yanking does NOT delete — existing users can still build. It only prevents new `Cargo.lock` entries.

## Troubleshooting

| Error | Fix |
|---|---|
| `the remote server responded with an error: crate name is already taken` | Pick a different `name` in Cargo.toml |
| `no upload token found` | Run `cargo login <token>` again |
| `failed to verify package tarball` | `cargo clean && cargo publish` |
| `missing field: description` | Add `description = "..."` to Cargo.toml |
| `missing field: license` | Add `license = "MIT OR Apache-2.0"` |
