terraform {
  backend "s3" {
    bucket = "unveil-terraform-state"
    key    = "unveil.tfstate"
    region = "eu-west-2"
  }

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 4.0"
    }
    cloudinit = {
      source  = "hashicorp/cloudinit"
      version = "~> 2.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.0"
    }
  }

  required_version = ">= 0.14.9"
}

provider "aws" {
  region = "eu-west-2"
}

module "shuttle" {
  source = "./modules/shuttle"

  api_fqdn             = "api.shuttle.rs"
  proxy_fqdn           = "shuttleapp.rs"
  postgres_password    = var.postgres_password
  shuttle_admin_secret = var.shuttle_admin_secret
}
