data "aws_caller_identity" "current" {}

locals {
  account_id   = data.aws_caller_identity.current.account_id
  data_dir     = "/opt/shuttle"
  docker_image = "public.ecr.aws/shuttle/backend"
}

resource "random_string" "initial_key" {
  length  = 16
  special = false
  lower   = true
  number  = true
  upper   = true
}
