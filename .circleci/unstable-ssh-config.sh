#! /usr/bin/env sh

set -ue

# add our config to the .ssh/config in circleci
cat >> $HOME/.ssh/config <<- EOF
Host *.shuttle.internal
    ProxyJump 3.11.51.209
    User ec2-user
    ProxyJump ec2-user@3.11.51.209
EOF
