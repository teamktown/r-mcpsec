#!/bin/bash
# This script is used to bootstrap the development container environment.
set -e
# Ensure the script is run from the correct directory
cd "$(dirname "$0")/../.."  
echo "Preparing toolchains.."
echo "Checking for Claude Code CLI"

#check if latest claude-code is installed
if ! command -v claude &> /dev/null; then
    echo "Claude CLI not found, installing..."
    npm install -g @anthropic-ai/claude-code

else
    echo "Claude CLI installed in "`which claude`", checking version...one moment please"

    claude --version
    exit 0
fi
echo "Run claude, login, use your subscription NOT API key, ctrl-c 2x to exit and then run these next steps"