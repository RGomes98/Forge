# Forge

A high-performance, asynchronous HTTP framework for Rust, powered by `io_uring` and the `Monoio` runtime for ultra-low latency.

## Platform Support

**Forge is a Linux-exclusive framework.** It is built natively for `io_uring` and does not provide fallbacks for other event loops or operating systems.

- **Linux:** Required (Kernel 5.10+ recommended).
- **Windows:** Not supported.
- **macOS:** Not supported.

## Getting Started

### Install Rust

Before you can build and run the project, you'll need to have the Rust compiler and Cargo (Rust's package manager and build tool) installed on your system. If you don't have Rust installed, follow these steps:

1. Visit [https://www.rust-lang.org/](https://www.rust-lang.org/) and follow the installation instructions for your operating system.
2. Install Rust directly via the command line:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. After installation, verify that Rust is installed by running:

   ```bash
   rustc --version
   ```

   This should output the installed version of the Rust compiler.

### Clone the repository

Clone the repository and navigate to the project folder:

```bash
git clone https://github.com/RGomes98/Forge.git
cd Forge
```

### Database Setup

To quickly spin up a `PostgreSQL` instance with the correct credentials, use the provided `docker-compose` file:

```bash
docker compose up -d
```

> **This starts a PostgreSQL 16 instance using the default credentials from `config.toml`.**

### Configure environment variables

1. Open the `config.toml` file located in the `./cargo` folder.
2. Set the `THREADS`, `PORT`, `HOST`, `DB_URL`, `DB_THREADS` and `DB_INFLIGHT_PER_CONN` variables according to your preferred configuration. By default, they are set to:

```toml
[env]
THREADS="0"
PORT="8080"
HOST="0.0.0.0"
DB_URL="postgresql://forge-example:forge-example@localhost:5432/forge-example"
DB_THREADS="8"
DB_INFLIGHT_PER_CONN="32"
```

### Build and run the server

Once the environment variables are configured, build and run the server using:

```bash
cargo run
```

The server will start on the specified host and port (default: `http://0.0.0.0:8080`).

### Build and Run in Release Mode

For maximum performance, use release mode. `cargo run` automatically injects variables from `.cargo/config.toml`:

```bash
cargo run --release
```

To run the binary independently, you must provide the environment variables manually as it will ignore the `.cargo/config.toml` file:

> **Compiles the project in release mode.**

```bash
cargo build --release
```

> **Example of passing custom environment variables directly to the binary.**

```bash
DB_URL="postgresql://user:pass@localhost:5432/db" ./target/release/forge-example
```

## Contributing

Feel free to open issues or contribute with improvements. Pull requests are welcome!

## License

Forge is distributed under the terms of the MIT License.

See [LICENSE](./LICENSE) for details.
