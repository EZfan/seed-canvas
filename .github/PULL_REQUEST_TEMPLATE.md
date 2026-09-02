# Pull Request Checklist

Thanks for the contribution! Please confirm the following before requesting
review.

- [ ] I have read [`CONTRIBUTING.md`](./CONTRIBUTING.md) and the
      [Code of Conduct](./CODE_OF_CONDUCT.md).
- [ ] `cargo fmt --all` produced no diff.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes locally.
- [ ] I have added tests covering my change.
- [ ] If the change affects the public API, I have updated the rustdoc
      comments and the `docs/` site.
- [ ] I have added a CHANGELOG entry under "Unreleased".
- [ ] My change does not introduce new dependencies without prior discussion.

## Summary

<!-- One sentence describing the change. -->

## Related Issues

<!-- Link the issue(s) this PR closes or addresses, e.g. "Closes #123". -->

## Testing

<!-- How did you verify the change? Manual commands, screenshots, etc. -->