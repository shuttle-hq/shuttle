variable "registry" {
  default = "public.ecr.aws/shuttle"
}

variable "context" {
  default = "."
}

target "api" {
  dockerfile = "Containerfile"
  context = "${context}"
  tags = ["${registry}/api"]
  args = {
    crate = "shuttle-api"
  }
}

target "provisioner" {
  dockerfile = "Containerfile"
  context = "${context}"
  tags = ["${registry}/provisioner"]
  args = {
    crate = "shuttle-provisioner"
  }
}
