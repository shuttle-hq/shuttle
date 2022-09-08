data "aws_caller_identity" "current" {}

locals {
  account_id               = data.aws_caller_identity.current.account_id
  data_dir                 = "/opt/shuttle"
  docker_backend_image     = "public.ecr.aws/shuttle/api"
  docker_provisioner_image = "public.ecr.aws/shuttle/provisioner"
}
