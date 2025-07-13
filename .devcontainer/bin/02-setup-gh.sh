#!/bin/bash

# install gh based on https://github.com/cli/cli/blob/trunk/docs/install_linux.md
set -e

echo "Installing GitHub CLI..."
#check if gh is installed and if not do the following

if ! command -v gh &> /dev/null; then

(type -p wget >/dev/null || (sudo apt update && sudo apt install wget -y)) \
	&& sudo mkdir -p -m 755 /etc/apt/keyrings \
	&& out=$(mktemp) && wget -nv -O$out https://cli.github.com/packages/githubcli-archive-keyring.gpg \
	&& cat $out | sudo tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null \
	&& sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
	&& sudo mkdir -p -m 755 /etc/apt/sources.list.d \
	&& echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
	&& sudo apt update \
	&& sudo apt install gh -y

else
    echo "Already installed in "`which gh`" version and status: "
    gh --version
echo""
    echo "Your status: "
    gh auth status || echo "You may need to run 'gh auth login' to authenticate."

fi
echo ""
echo ""

echo " ## IT IS STRONGLY ADVISED TO USE A PERSONAL ACCESS TOKEN!"
echo "Why? Because it limits access to just the repositories you want to work with, and not your entire account."
echo
echo "You can generate a token at:"
echo " https://github.com/settings/personal-access-tokens"
echo ""
echo 'To add the token, gh auth login -> choose github.com ->choose ssh -> generate new ssh key (select no) -> Paste your token '

