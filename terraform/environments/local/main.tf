terraform {
  required_providers {
    local = {
      source  = "hashicorp/local"
      version = "~> 2.4.0"
    }
    null = {
      source  = "hashicorp/null"
      version = "~> 3.2.0"
    }
  }
}

locals {
  service_name = "images-api"
  user         = "jeffbridwell"
  working_dir  = "/Users/${local.user}/CascadeProjects/images-api"
  binary_path  = "${local.working_dir}/target/release/images-api"
}

# Create logs directory
resource "null_resource" "create_logs_dir" {
  provisioner "local-exec" {
    command = "mkdir -p ${local.working_dir}/logs"
  }
}

# Build release binary
resource "null_resource" "build_release" {
  provisioner "local-exec" {
    command = "cargo build --release"
    working_dir = local.working_dir
  }
}

# Run the service directly
resource "null_resource" "run_service" {
  depends_on = [null_resource.build_release, null_resource.create_logs_dir]
  
  provisioner "local-exec" {
    command = "nohup ${local.binary_path} > ${local.working_dir}/logs/stdout.log 2> ${local.working_dir}/logs/stderr.log & echo $! > /Users/jeffbridwell/CascadeProjects/images-api/api.pid"
    working_dir = local.working_dir
    environment = {
      RUST_LOG = "debug"
      IMAGES_DIR = "/Volumes/VideosNew/Models"
    }
  }

  # Cleanup on destroy
  provisioner "local-exec" {
    when    = destroy
    command = "kill -9 $(cat /Users/jeffbridwell/CascadeProjects/images-api/api.pid) || true && rm -f /Users/jeffbridwell/CascadeProjects/images-api/api.pid"
  }
}

output "health_check_endpoint" {
  value = "http://localhost:8081/health"
}

output "logs_location" {
  value = "${local.working_dir}/logs"
}
