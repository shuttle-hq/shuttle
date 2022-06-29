variable "registry" {
  default = "public.ecr.aws/j5v8s8v8"
}

variable "context" {
  default = "."
}

target "backend" {
  dockerfile = "Containerfile"
  context = "${context}"
  tags = ["${registry}/backend"]
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
