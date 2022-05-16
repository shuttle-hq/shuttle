variable "postgres_password" {
  type        = string
  description = "Root password for postgres instance"
}

variable "shuttle_admin_secret" {
  type        = string
  description = "Secret for the shuttle admin user"
}
