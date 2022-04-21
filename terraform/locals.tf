locals {
  data_dir     = "/opt/shuttle"
  docker_image = "public.ecr.aws/d7w6e9t1/backend"
}

resource "random_string" "initial_key" {
  length  = 16
  special = false
  lower   = true
  number  = true
  upper   = true
}
