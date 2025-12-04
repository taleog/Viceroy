#!/bin/bash
# Setup script for Viceroy development environment
# Run this after cloning the repository

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🔧 Setting up Viceroy development environment..."

# Install git hooks
echo "📎 Installing git hooks..."
HOOKS_DIR="$PROJECT_ROOT/.git/hooks"
mkdir -p "$HOOKS_DIR"

# Install pre-commit hook
if [ -f "$SCRIPT_DIR/pre-commit" ]; then
    cp "$SCRIPT_DIR/pre-commit" "$HOOKS_DIR/pre-commit"
    chmod +x "$HOOKS_DIR/pre-commit"
    echo "  ✓ Pre-commit hook installed"
fi

# Install commit-msg hook for conventional commits
cat > "$HOOKS_DIR/commit-msg" << 'EOF'
#!/bin/bash
# Commit message validation hook
# Enforces Conventional Commits format

commit_msg_file=$1
commit_msg=$(cat "$commit_msg_file")

# Skip merge commits
if echo "$commit_msg" | grep -qE "^Merge"; then
    exit 0
fi

# Conventional commit pattern
pattern="^(feat|fix|docs|style|refactor|test|chore|perf|ci|build|revert)(\([a-z0-9_-]+\))?: .{1,}"

if ! echo "$commit_msg" | grep -qE "$pattern"; then
    echo "❌ Invalid commit message format"
    echo ""
    echo "Commit message must follow Conventional Commits:"
    echo "  <type>(<scope>): <description>"
    echo ""
    echo "Types: feat, fix, docs, style, refactor, test, chore, perf, ci, build, revert"
    echo ""
    echo "Examples:"
    echo "  feat(search): add contact search support"
    echo "  fix(clipboard): resolve duplicate detection"
    echo "  docs(readme): update installation guide"
    echo ""
    exit 1
fi

exit 0
EOF
chmod +x "$HOOKS_DIR/commit-msg"
echo "  ✓ Commit-msg hook installed"

# Check Rust toolchain
echo ""
echo "🦀 Checking Rust toolchain..."
if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    echo "  ✓ $RUST_VERSION"
else
    echo "  ❌ Rust not found. Install from https://rustup.rs/"
    exit 1
fi

# Check for required components
echo ""
echo "📦 Checking Rust components..."
if rustup component list --installed | grep -q rustfmt; then
    echo "  ✓ rustfmt installed"
else
    echo "  Installing rustfmt..."
    rustup component add rustfmt
fi

if rustup component list --installed | grep -q clippy; then
    echo "  ✓ clippy installed"
else
    echo "  Installing clippy..."
    rustup component add clippy
fi

# Create config directory if it doesn't exist
echo ""
echo "📁 Checking config directory..."
CONFIG_DIR="$HOME/.config/viceroy"
if [ ! -d "$CONFIG_DIR" ]; then
    mkdir -p "$CONFIG_DIR"
    echo "  ✓ Created $CONFIG_DIR"
else
    echo "  ✓ Config directory exists"
fi

# Build the project to verify setup
echo ""
echo "🔨 Building project to verify setup..."
cd "$PROJECT_ROOT"
cargo build 2>&1 | tail -5

echo ""
echo "✅ Development environment setup complete!"
echo ""
echo "📚 Quick reference:"
echo "   make run      - Run the app"
echo "   make test     - Run tests"
echo "   make fmt      - Format code"
echo "   make lint     - Run clippy"
echo "   make help     - Show all commands"
echo ""
echo "📝 Git hooks installed:"
echo "   pre-commit    - Format check, clippy, changelog reminder"
echo "   commit-msg    - Conventional commit message validation"
