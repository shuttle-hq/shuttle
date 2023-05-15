#! /usr/bin/env sh

set -ue

# add our config to the .ssh/config in circleci
cat >> $HOME/.ssh/config <<- EOF
Host admin
    HostName 18.132.154.166
    User ec2-user

Host *.shuttle.internal
    User ec2-user
    ProxyJump ec2-user@admin
EOF
