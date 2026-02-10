# Forge

Forge is a lightweight HTTP framework built in Rust using Monoio.

## Getting Started

### Install Rust

Before you can build and run the project, you'll need to have the Rust compiler and Cargo (Rust's package manager and build tool) installed on your system. If you don't have Rust installed, follow these steps:

1. Visit [https://www.rust-lang.org/](https://www.rust-lang.org/) and follow the installation instructions for your operating system.
2. Alternatively, you can install Rust using the following command:

   For Linux/macOS:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

   For Windows, use the [Rust installer](https://www.rust-lang.org/tools/install).

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

### Configure environment variables

1. Open the `config.toml` file located in the `./cargo` folder.
2. Set the `PORT`, `HOST` and `RUST_LOG` variables according to your preferred configuration. By default, they are set to:

```toml
[env]
THREADS="0"
PORT="8080"
HOST="0.0.0.0"
RUST_LOG="debug"
```

### Build and run the server

Once the environment variables are configured, build and run the server using:

```bash
cargo run
```

The server will start on the specified host and port (default: `http://0.0.0.0:8080`).

### Build in Release (better performance)

To run the server with optimizations enabled, build it in release mode:

```bash
cargo build --release
```

Then run the generated binary:

```bash
./target/release/forge-example
```

## Contributing

Feel free to open issues or contribute with improvements. Pull requests are welcome!

## License

Forge is distributed under the terms of the MIT License.

See [LICENSE](./LICENSE) for details.
