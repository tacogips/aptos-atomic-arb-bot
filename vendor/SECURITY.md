# TLS root data security backport

The legacy webpki-roots crates retain their upstream public API, package version,
license, and runtime dependencies. Their complete trust-anchor arrays are replaced
by Mozilla's current list from **webpki-roots 1.0.11** (118 roots), mitigating
RUSTSEC-2026-0381 and removing the additional C1/C3 emSign roots.

Source: https://github.com/rustls/webpki-roots/releases/tag/v/1.0.11
Official crate: https://static.crates.io/crates/webpki-roots/webpki-roots-1.0.11.crate
Archive SHA-256: f5f280bbb2b287bc8e7de7e3852945a0ff50d1bbcb5ed42b6c7a568c546df171

The mechanical adaptation unwraps `Der::from_slice`, renames
`subject_public_key_info` to `spki`, and retains the legacy array wrapper and
TrustAnchor type. Subject, public-key and name-constraint bytes are unchanged.
All root additions, removals, constraints and upstream certificate comments are
preserved. Upstream generators and development tools are not vendored.

This is a trust-data backport, not a claim that the legacy TLS library is fully
current. Version-only scanners may still flag the original crate version.
Do not suppress unrelated findings. A future TLS stack migration can remove these
Cargo patches. Rebuild and redeploy binaries to use the refreshed trust data.
