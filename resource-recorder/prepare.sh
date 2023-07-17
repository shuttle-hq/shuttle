#!/bin/bash

###############################################################################
# This file is used by our common Containerfile incase the container for this #
# service might need some extra preparation steps for its final image         #
###############################################################################

# Stuff that depends on local source files
if [ "$1" = "--after-src" ]; then
    exit 0
fi

# Nothing to prepare in container image here
