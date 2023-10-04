#!/usr/bin/env sh

##############################################################################################
# This file is run by Containerfile for extra preparation steps for this crate's final image #
##############################################################################################

# We're using rustls for the async-stripe crate and that needs certificates installed.
apt install ca-certificates