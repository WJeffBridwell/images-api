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
  binary_path  = "${local.working_dir}/target/release/image-api"
}

# Create LaunchAgent plist file for auto-start
resource "local_file" "launch_agent" {
  filename = "/Users/${local.user}/Library/LaunchAgents/com.${local.user}.${local.service_name}.plist"
  content  = <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.${local.user}.${local.service_name}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${local.binary_path}</string>
    </array>
    <key>WorkingDirectory</key>
    <string>${local.working_dir}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>${local.working_dir}/logs/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>${local.working_dir}/logs/stderr.log</string>
</dict>
</plist>
EOF
}

# Ensure logs directory exists
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

# Health check resource
resource "null_resource" "health_check" {
  depends_on = [null_resource.build_release, local_file.launch_agent]

  # Runs health check after creating resources
  provisioner "local-exec" {
    command = <<EOF
      # Wait for service to start
      sleep 5
      
      # Check if service is running
      if ! curl -s http://localhost:8081/health > /dev/null; then
        echo "Health check failed: Service is not responding"
        exit 1
      fi
      
      # Check if images directory exists
      if [ ! -d "${local.working_dir}/images" ]; then
        echo "Health check failed: Images directory not found"
        exit 1
      fi
      
      echo "Health checks passed successfully"
    EOF
  }
}

# Output important information
output "service_status" {
  value = "LaunchAgent created at ~/Library/LaunchAgents/com.${local.user}.${local.service_name}.plist"
}

output "health_check_endpoint" {
  value = "http://localhost:8081/health"
}

output "logs_location" {
  value = "${local.working_dir}/logs"
}
