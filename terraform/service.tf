resource "aws_ecs_cluster" "backend" {
  name = "unveil-ecs-cluster"
}

resource "aws_cloudwatch_log_group" "backend" {
  name = "unveil-backend"
}

resource "aws_ecs_task_definition" "api" {
  family = "backend"

  requires_compatibilities = [ "FARGATE" ]

  network_mode = "awsvpc"

  cpu = var.service_cpu
  memory = var.service_memory

  execution_role_arn = aws_iam_role.backend.arn

  // TODO: demote task_role
  task_role_arn = aws_iam_role.backend.arn

  volume {
    name = "user-data"

    efs_volume_configuration {
      file_system_id          = aws_efs_file_system.user_data.id
      root_directory          = "/"
      transit_encryption      = "DISABLED"
      authorization_config {
        iam             = "DISABLED"
      }
    }
  }

  container_definitions = jsonencode([
    {
      name = "backend"

      image = aws_ecr_repository.backend.repository_url,

      cpu = var.service_cpu

      memory = var.service_memory

      essential = true

      user = "root"

      command = [
        "--path",
        "/opt/unveil/crates",
        "--bind-addr",
        "0.0.0.0",
        "--api-port",
        tostring(var.api_container_port),
        "--proxy-port",
        tostring(var.proxy_container_port)
      ]

      healthCheck = {
        command = ["CMD", "curl", "http://localhost:${tostring(var.api_container_port)}/status"]
      }

      // TODO: Do we need this?
      portMappings = [
        {
          containerPort = var.api_container_port
          hostPort = var.api_container_port
        },
        {
          containerPort = var.proxy_container_port
          hostPort = var.proxy_container_port
        }
      ]

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          awslogs-group = aws_cloudwatch_log_group.backend.name
          awslogs-stream-prefix = "awslogs-backend-container"
          awslogs-region = "eu-west-2"
        }
      }

      mountPoints = [
        {
          sourceVolume = "user-data"
          containerPath = "/opt/unveil/crates"
        }
      ]
    }
  ])
}

resource "aws_ecs_service" "api" {
  name = "backend"
  cluster = aws_ecs_cluster.backend.id
  task_definition = aws_ecs_task_definition.api.arn
  desired_count = var.desired_count

  launch_type = "FARGATE"

  network_configuration {
    subnets = [aws_subnet.backend_a.id, aws_subnet.backend_b.id]
    security_groups = [aws_security_group.unreasonable.id]
    assign_public_ip = true
  }
  
  load_balancer {
    target_group_arn = aws_lb_target_group.api.arn
    container_name = "backend"
    container_port = var.api_container_port
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.user.arn
    container_name = "backend"
    container_port = var.proxy_container_port
  }
}
