#!/bin/bash

# Claude Token Monitor Security Audit Script
# This script runs comprehensive security checks on the codebase

set -euo pipefail

echo "🔒 Starting Claude Token Monitor Security Audit..."
echo "=================================================="

# Check if cargo-audit is installed
if ! command -v cargo-audit &> /dev/null; then
    echo "📦 Installing cargo-audit..."
    cargo install cargo-audit
fi

# Check if cargo-outdated is installed
if ! command -v cargo-outdated &> /dev/null; then
    echo "📦 Installing cargo-outdated..."
    cargo install cargo-outdated
fi

# 1. Run dependency vulnerability scan
echo "🔍 Running dependency vulnerability scan..."
cargo audit

# 2. Check for outdated dependencies
echo "🔍 Checking for outdated dependencies..."
cargo outdated

# 3. Run clippy with security lints
echo "🔍 Running security-focused clippy lints..."
cargo clippy -- \
    -W clippy::integer_overflow \
    -W clippy::panic \
    -W clippy::unwrap_used \
    -W clippy::expect_used \
    -W clippy::indexing_slicing \
    -W clippy::mem_forget \
    -W clippy::debug_assert_with_mut_call \
    -W clippy::exit \
    -W clippy::filetype_is_file \
    -W clippy::float_cmp \
    -W clippy::lossy_float_literal \
    -W clippy::mutex_atomic \
    -W clippy::path_buf_push_overwrite

# 4. Check for unsafe code
echo "🔍 Scanning for unsafe code blocks..."
if rg -n "unsafe" src/; then
    echo "⚠️  WARNING: Unsafe code blocks found!"
else
    echo "✅ No unsafe code blocks found"
fi

# 5. Check for potential secrets in code
echo "🔍 Scanning for potential secrets..."
if rg -i "(password|secret|key|token|api)" src/ --type rust; then
    echo "⚠️  WARNING: Potential secrets found in code!"
else
    echo "✅ No obvious secrets found in code"
fi

# 6. Check for hardcoded paths
echo "🔍 Checking for hardcoded paths..."
if rg -n "(?:/home/|/usr/|/var/|/etc/|C:\\\\|D:\\\\)" src/; then
    echo "⚠️  WARNING: Hardcoded paths found!"
else
    echo "✅ No hardcoded paths found"
fi

# 7. Generate dependency tree
echo "🔍 Generating dependency tree..."
cargo tree > dependency-tree.txt
echo "📝 Dependency tree saved to dependency-tree.txt"

# 8. Check for dependency licenses
echo "🔍 Checking dependency licenses..."
cargo tree --format "{p} {l}" | grep -E "(GPL|AGPL|LGPL|CDDL)" || echo "✅ No copyleft licenses found"

# 9. Run tests with security features
echo "🔍 Running tests with security features..."
RUST_BACKTRACE=1 cargo test --release

# 10. Check binary size and dependencies
echo "🔍 Analyzing binary size and dependencies..."
cargo build --release
ls -lh target/release/claude-token-monitor

echo "=================================================="
echo "✅ Security audit complete!"
echo "📋 Check the output above for any security issues"
echo "📝 See SECURITY_ANALYSIS.md for detailed security information"