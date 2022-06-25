variable "registry" {
  default = "public.ecr.aws/shuttle"
}

variable "context" {
  default = "."
}

target "build" {
  dockerfile = "docker/build.Containerfile"
  context = "${context}"
}

target "common" {
  dockerfile = "docker/common.Containerfile"
  context = "${context}"
}

target "api" {
  dockerfile = "docker/runtime.Containerfile"
  context = "${context}"
  contexts = {
    shuttle-build = "target:build"
    shuttle-common = "target:common"
  }
  tags = ["${registry}/backend"]
  platforms = ["linux/amd64"]
  args = {
    crate = "shuttle-api"
  }
}

target "provisioner" {
  dockerfile = "docker/runtime.Containerfile"
  context = "${context}"
  contexts = {
    shuttle-build = "target:build"
    shuttle-common = "target:common"
  }
  tags = ["${registry}/provisioner"]
  platforms = ["linux/amd64"]
  args = {
    crate = "shuttle-provisioner"
  }
}
