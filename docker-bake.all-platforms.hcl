# To build images for all supported platforms
#
#
# You will need a builder with support for `linux/arm64` and
# `linux/amd64`:
#
# ```bash
# docker buildx create --platform linux/arm64,linux/amd64 --use --name xbuilder
# ```
#
# and to install the qemu emulators for the missing platforms, e.g.
#
# ```bash
# docker run -it --rm --privileged tonistiigi/binfmt --install arm64
# ```
#
# You can then build all images with:
#
# ```bash
# docker buildx bake -f docker-bake.hcl -f docker-bake.all-platforms.hcl provisioner api
# ```bash

target "provisioner" {
  platforms = ["linux/arm64", "linux/amd64"]
}

target "api" {
  platforms = ["linux/arm64", "linux/amd64"]
}
