# Pull Request

## Description

<!-- Provide a clear and concise description of your changes -->

## Type of Change

<!-- Check all that apply -->

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring
- [ ] Test coverage improvement
- [ ] CI/CD enhancement

## Related Issues

<!-- Link to related issues using #issue_number -->

Fixes #
Relates to #

## Motivation and Context

<!-- Why is this change required? What problem does it solve? -->

## Changes Made

<!-- List the key changes made in this PR -->

- 
- 
- 

## Testing

<!-- Describe the tests you ran and how to reproduce them -->

### Test Environment
- **OS:** <!-- e.g., Ubuntu 22.04, macOS 14, Windows 11 -->
- **Rust version:** <!-- e.g., 1.75.0 -->
- **Python version (if applicable):** <!-- e.g., 3.11.4 -->

### Test Commands
```bash
# Commands to run tests
cargo test --all-features
pytest tests/ -v
```

### Test Results
<!-- Paste test output or link to CI run -->

```
# Test output
```

## Performance Impact

<!-- Does this change affect performance? Provide benchmark results if applicable -->

- [ ] No performance impact
- [ ] Performance improved
- [ ] Performance degraded (justified below)

<!-- If performance changed, provide data -->

## Breaking Changes

<!-- List any breaking changes and migration steps -->

- [ ] No breaking changes
- [ ] Breaking changes (documented below)

<!-- If breaking changes, describe them -->

## Documentation

<!-- Check all that apply -->

- [ ] Code comments added/updated
- [ ] README updated
- [ ] API documentation updated (docs/reference/API_REFERENCE.md)
- [ ] Changelog entry added (CHANGELOG.md)
- [ ] Examples added/updated (examples/)
- [ ] Migration guide provided (if breaking changes)

## Checklist

<!-- Check all that apply. Replace [ ] with [x] to check -->

### Code Quality
- [ ] My code follows the project's style guidelines (see docs/guides/CONTRIBUTING.md)
- [ ] I have performed a self-review of my own code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have run `cargo fmt` and `cargo clippy`
- [ ] My changes generate no new warnings

### Testing
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] New and existing unit tests pass locally with my changes
- [ ] I have tested on multiple platforms (if applicable)

### Documentation
- [ ] I have updated relevant documentation
- [ ] I have added/updated examples demonstrating the changes
- [ ] I have updated the CHANGELOG.md

### Compliance
- [ ] My changes maintain `no_std` compatibility (if applicable)
- [ ] My changes maintain deterministic behavior (Q16.16 fixed-point)
- [ ] I have verified no unwrap() calls were added (except in tests)
- [ ] I have checked for potential security issues

### Dependencies
- [ ] No new dependencies added
- [ ] New dependencies are justified and documented
- [ ] Dependency versions are pinned appropriately

## Screenshots (if applicable)

<!-- Add screenshots for UI changes or visualizations -->

## Additional Notes

<!-- Any additional information that reviewers should know -->

---

## Reviewer Notes

<!-- For reviewers: Add notes here during review -->

### Review Checklist
- [ ] Code follows project conventions
- [ ] Tests are adequate and pass
- [ ] Documentation is clear and complete
- [ ] No security concerns
- [ ] Performance impact is acceptable
- [ ] Breaking changes are justified and documented

### Suggested Improvements
<!-- List any suggestions for improvement -->

---

**Note to Contributors:** Please read [docs/guides/CONTRIBUTING.md](../../docs/guides/CONTRIBUTING.md) before submitting. Thank you for your contribution!
