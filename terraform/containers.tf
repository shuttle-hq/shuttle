provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"
}

resource "aws_ecrpublic_repository" "backend" {
  provider = aws.us_east_1

  repository_name = "backend"

  catalog_data {
    architectures     = ["x86-64"]
    operating_systems = ["Linux"]
  }
}
