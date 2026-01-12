# Contributing to Ruster Shield

Thanks for your interest in contributing! 🎉

## 🌿 Branch Strategy

We use **branch protection** on `master`. All contributions must go through Pull Requests.

```
master (protected)     <- Production, no direct pushes
  └── develop          <- Integration branch
       └── feature/*   <- New features
       └── fix/*       <- Bug fixes
       └── docs/*      <- Documentation
```

## 🚀 How to Contribute

### 1. Fork & Clone

```bash
git clone https://github.com/YOUR_USERNAME/Rust-REVM.git
cd Rust-REVM
```

### 2. Create a New Branch

```bash
# For new features
git checkout -b feature/feature-name

# For bug fixes
git checkout -b fix/bug-name

# For documentation
git checkout -b docs/update-readme
```

⚠️ **DO NOT** push directly to `master`!

### 3. Develop & Test

```bash
# Make sure code compiles
cargo check

# Run tests
cargo test

# Ensure clippy passes (REQUIRED)
cargo clippy --all-targets --all-features -- -D warnings

# Format code
cargo fmt
```

### 4. Commit with Clear Messages

```bash
git add .
git commit -m "feat: add caching layer for RPC optimization"
```

Commit message format:
- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation
- `refactor:` - Code refactoring
- `test:` - Adding tests
- `chore:` - Maintenance

### 5. Push & Create Pull Request

```bash
git push origin feature/feature-name
```

Then open GitHub and create a Pull Request to `develop` or `master` branch.

## ✅ PR Checklist

- [ ] Code compiles without errors (`cargo check`)
- [ ] All tests pass (`cargo test`)
- [ ] Clippy passes without warnings (`cargo clippy -- -D warnings`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Commit messages follow the format
- [ ] PR description explains the changes

## 🔒 Branch Protection Rules

The `master` branch is protected with:
- ❌ No direct pushes allowed
- ✅ Must go through Pull Request
- ✅ Must pass CI checks
- ✅ Minimum 1 approval (if reviewers available)

## 📝 Code Style

- Use `cargo fmt` for formatting
- Follow Rust idioms and best practices
- Add documentation for public functions
- Write tests for new features

## 🐛 Reporting Bugs

Open an [Issue](https://github.com/nirvagold/Rust-REVM/issues) with:
- Bug description
- Steps to reproduce
- Expected vs actual behavior
- Environment (OS, Rust version)

## 💡 Feature Requests

Open an [Issue](https://github.com/nirvagold/Rust-REVM/issues) with the `enhancement` label.

---

Thanks for contributing! 🦀
