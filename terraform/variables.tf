variable "api_container_port" {
  type    = number
  default = 8001
}

variable "api_fqdn" {
  type        = string
  description = "Fully qualified domain name where the api will be reachable at"
}

variable "postgres_container_port" {
  type    = number
  default = 5432
}

variable "postgres_password" {
  type        = string
  description = "Root password for postgres instance"
}

variable "proxy_container_port" {
  type    = number
  default = 8000
}

variable "proxy_fqdn" {
  type        = string
  description = "The top level domain where deployed services can be reached at"
}

variable "shuttle_admin_secret" {
  type        = string
  description = "Secret for the shuttle admin user"
}

variable "availability_zone_1" {
  type        = string
  description = "First availability zone for load balancer"
}

variable "availability_zone_2" {
  type        = string
  description = "Second availability zone for load balancer"
}
