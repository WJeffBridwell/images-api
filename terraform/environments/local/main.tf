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
  dashboard_dir = "${local.working_dir}/dashboard"
  mongodb_port = 27017
  dashboard_port = 8502
  pip_packages = [
    "streamlit==1.41.1",
    "pandas==2.2.3",
    "plotly==5.24.1",
    "fastapi==0.115.6",
    "sqlalchemy==2.0.37",
    "pymongo==4.6.1"
  ]
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

# Install Python dependencies
resource "null_resource" "install_python_deps" {
  provisioner "local-exec" {
    command = <<-EOT
      python3 -m venv venv
      source venv/bin/activate
      pip install ${join(" ", local.pip_packages)}
    EOT
    working_dir = local.dashboard_dir
  }
}

# Start Python Dashboard
resource "null_resource" "start_dashboard" {
  depends_on = [null_resource.create_logs_dir, null_resource.install_python_deps]

  provisioner "local-exec" {
    command = <<-EOT
      echo "Starting dashboard setup..."
      
      # Ensure we're in the right directory
      pwd
      ls -la
      
      # Check if venv exists and is properly setup
      if [ ! -d "venv" ]; then
        echo "Virtual environment not found!"
        exit 1
      fi
      
      # Activate venv and verify Python path
      source venv/bin/activate
      which python
      python --version
      
      # Check if streamlit is installed
      pip list | grep streamlit
      
      # Check if app.py exists
      if [ ! -f "app.py" ]; then
        echo "app.py not found!"
        exit 1
      fi
      
      echo "Starting streamlit..."
      nohup streamlit run app.py --server.port ${local.dashboard_port} --server.headless true > ${local.working_dir}/logs/dashboard-stdout.log 2> ${local.working_dir}/logs/dashboard-stderr.log & 
      echo $! > ${local.working_dir}/dashboard.pid
      
      # Wait a bit and check if process is still running
      sleep 2
      if ! ps -p $(cat ${local.working_dir}/dashboard.pid) > /dev/null; then
        echo "Dashboard process died immediately. Checking logs:"
        tail -n 50 ${local.working_dir}/logs/dashboard-stderr.log
        exit 1
      fi
      
      echo "Dashboard started successfully!"
    EOT
    working_dir = local.dashboard_dir
  }

  # Cleanup on destroy
  provisioner "local-exec" {
    when    = destroy
    command = "kill -9 $(cat /Users/jeffbridwell/CascadeProjects/images-api/dashboard.pid) || true && rm -f /Users/jeffbridwell/CascadeProjects/images-api/dashboard.pid"
  }
}

# Run MongoDB
resource "null_resource" "run_mongodb" {
  depends_on = [null_resource.create_logs_dir]
  
  provisioner "local-exec" {
    command = "nohup mongod --port ${local.mongodb_port} > ${local.working_dir}/logs/mongo-stdout.log 2> ${local.working_dir}/logs/mongo-stderr.log & echo $! > ${local.working_dir}/mongo.pid"
    working_dir = local.working_dir
  }

  # Cleanup on destroy
  provisioner "local-exec" {
    when    = destroy
    command = "kill -9 $(cat /Users/jeffbridwell/CascadeProjects/images-api/mongo.pid) || true && rm -f /Users/jeffbridwell/CascadeProjects/images-api/mongo.pid"
  }
}

# Run the service directly
resource "null_resource" "run_service" {
  depends_on = [null_resource.build_release, null_resource.create_logs_dir, null_resource.run_mongodb]
  
  provisioner "local-exec" {
    command = "nohup ${local.binary_path} > ${local.working_dir}/logs/node-stdout.log 2> ${local.working_dir}/logs/node-stderr.log & echo $! > ${local.working_dir}/api.pid"
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

output "mongodb_endpoint" {
  value = "mongodb://localhost:${local.mongodb_port}"
}

output "dashboard_endpoint" {
  value = "http://localhost:${local.dashboard_port}"
}

output "logs_location" {
  value = "${local.working_dir}/logs"
}
