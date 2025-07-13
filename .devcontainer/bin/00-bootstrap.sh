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
    
fi
echo""
echo "Run claude, login, use your subscription NOT API key, ctrl-c 2x to exit and then run these next steps"
echo ""
echo "To set your timezone in the container, run:"
echo "sudo ln -sf /usr/share/zoneinfo/Etc/GMT+5 /etc/localtime"
echo ""
echo "Replace GMT+5 with your timezone, e.g., GMT+8 for Singapore, GMT+1 for London, etc."
echo "You can find your timezone at https://en.wikipedia.org/wiki/List_of_tz_database_time_zones"
