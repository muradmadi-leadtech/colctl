# colctl task runner. `just` on its own lists everything.
#
# The toolchain (rust, just and the audit tools) is pinned in mise.toml — run `mise install` once.
# The gate's recipes are in sadr.just, which `sadr init` rewrites. The project's own recipes go
# here, and one named like a recipe there replaces it.

set allow-duplicate-recipes

import 'sadr.just'

# List available recipes
default:
    @just --list --unsorted

# One-time setup: commit guard
setup:
    git config core.hooksPath .githooks

# The tests, with line coverage gated at 90%. Replaces sadr.just's `test` so `check` and CI use it.
# The fake `claude` in examples/ is test support, so it's left out of the measurement.
test:
    cargo llvm-cov --locked --fail-under-lines 90 --ignore-filename-regex '(examples|tests)/' -- --include-ignored

# An HTML coverage report in target/llvm-cov/html
coverage:
    cargo llvm-cov --locked --html --ignore-filename-regex '(examples|tests)/' -- --include-ignored
