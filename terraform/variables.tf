variable "service_cpu" {
  type = number
  default = 4096
}

variable "service_memory" {
  type = number
  default = 8192
}

variable "desired_count" {
  type = number
  default = 1
}

variable "api_container_port" {
  type = number
  default = 8001
}

variable "proxy_container_port" {
  type = number
  default = 8000
}
