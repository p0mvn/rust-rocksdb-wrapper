# Publishing the wrapper

The wrapper and native library are versioned separately. Release only packages
whose source changed. A Rust-only wrapper addition does not require a new
`zakura-librocksdb-sys` version.

Before the first workflow publication, a repository administrator must create
the `crates-io` GitHub Environment with required human reviewers and restrict it
to the default branch. A crates.io owner must configure Trusted Publishing for
`zakura-rocksdb` with repository `zakura-core/rust-rocksdb-wrapper`, workflow
`publish-crates.yml`, and environment `crates-io`. The workflow uses short-lived
OIDC credentials; it requires no stored registry token.

1. Merge the reviewed source, package version, lockfile, and changelog changes
   into `zakura-unity-0.24`.
2. Run `publish-crates.yml` from that branch with the exact package `version`
   and `mode=verify`. It checks the setter against a real database and builds
   the package against registry dependencies. Nothing is uploaded.
3. After verification passes, obtain explicit publication approval and dispatch
   the same workflow from the same commit with `mode=publish`. Verify that the
   environment approval request names that commit before approving it.
4. Confirm the version appears in the crates.io index, then update downstream
   manifests and lockfiles to the published version. Do not restore registry
   dependencies before the required API is available there.

The publish job uploads only `zakura-rocksdb`. A change to the native library
needs its own version bump, verification, and publication before a wrapper
depending on that version can publish.
