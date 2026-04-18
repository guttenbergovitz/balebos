#!/bin/bash
set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "🔍 Exporting data from Parquet storage..."
cargo run --release -- web export --output-dir "$SCRIPT_DIR/web/static/data"

echo "🏗️  Building Zola site..."
cd "$SCRIPT_DIR/web"
zola build

echo "✅ Build complete!"
echo "📁 Output: $SCRIPT_DIR/web/public/"
echo ""
echo "To preview locally:"
echo "  cd $SCRIPT_DIR/web"
echo "  zola serve"
