# Images API

A high-performance image serving API built with Rust and Actix Web.

## Features

- Fast image serving with async I/O
- Image metadata extraction
- Health check endpoint
- Comprehensive test suite including:
  - Unit tests
  - Integration tests
  - Performance benchmarks

## Prerequisites

- Rust (latest stable version)
- Cargo
- Terraform (for local development)

## Getting Started

1. Clone the repository:
```bash
git clone https://github.com/YOUR_USERNAME/images-api.git
cd images-api
```

2. Build the project:
```bash
cargo build
```

3. Run the tests:
```bash
./scripts/test.sh
```

4. Start the server:
```bash
cargo run
```

The server will start on `http://localhost:8081`

## API Endpoints

- `GET /health` - Health check endpoint
- `GET /images/{filename}` - Serve image files
- `GET /images/{filename}/info` - Return image metadata

## Development

### Project Structure
```
images-api/
├── src/
│   ├── lib.rs         # Library entry point
│   ├── main.rs        # Application entry point
│   ├── handlers.rs    # HTTP route handlers
│   └── startup.rs     # Server initialization
├── tests/
│   ├── unit/          # Unit test directory
│   ├── integration/   # Integration test directory
│   └── performance/   # Performance test directory
├── benches/           # Benchmarking tests
└── scripts/
    └── test.sh        # Test runner script
```

### Running Tests

The test suite includes:
- Unit tests
- Integration tests
- Performance benchmarks
- Code formatting checks
- Linting

Run all tests with:
```bash
./scripts/test.sh
```

Run benchmarks with:
```bash
cargo bench
```

## License

[MIT License](LICENSE)
