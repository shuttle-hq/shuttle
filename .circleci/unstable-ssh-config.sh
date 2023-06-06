#! /usr/bin/env sh

set -ue

# add our config to the .ssh/config in circleci
cat >> $HOME/.ssh/config <<- EOF
Host admin.unstable
    HostName 3.11.51.209
    User ec2-user

Host *.shuttle.internal
    User ec2-user
    ProxyJump ec2-user@admin.unstable
EOF
