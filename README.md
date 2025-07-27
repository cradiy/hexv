# hexv

A terminal-based hex viewer written in Rust with an interactive interface.

## Features

- **Interactive Hex Viewing**: Navigate through files using keyboard controls
- **Customizable Display**: Configure bytes per line (default: 16)
- **Flexible Starting Position**: Start viewing from any offset (decimal or hexadecimal)
- **Command Mode**: Execute commands for advanced navigation
- **Real-time Navigation**: Smooth scrolling through large files

## Usage

### Basic Usage

```bash
# View a file from the beginning
hexv myfile.bin

# Start from a specific offset (decimal)
hexv --start 100 myfile.bin

# Start from a hexadecimal offset
hexv --start 0x64 myfile.bin

# Customize bytes per line
hexv --bytes-per-line 8 myfile.bin
```

### Command Line Options

- `FILE`: Path to the file to view (required)
- `-s, --start <OFFSET>`: Starting offset (decimal or hex with 0x prefix, default: 0)
- `-w, --bytes-per-line <COUNT>`: Number of bytes to display per line (default: 16)

### Interactive Controls

Once the hex viewer is running, you can use the following keyboard controls:

#### Navigation
- **Arrow Keys**: Navigate through the hex data
- **Page Up/Page Down** / **h** / **l**: Move by pages (h for up, l for down)
- **Home/End**: Go to beginning/end of file
- **g**: Go to specific offset (enter command mode)

#### Modes
- **Normal Mode**: Default navigation mode
- **Command Mode**: Enter commands by typing `:` followed by the command

#### General
- **q**: Quit the application
- **Esc**: Return to normal mode (from command mode)

### Command Mode

Press `:` to enter command mode, then use these commands:

- `q` or `quit`: Exit the application
- `g <offset>`: Go to specific offset (supports hex with 0x prefix)

## Examples

```bash
# View a binary file
hexv /bin/ls

# Start viewing from byte 1024
hexv --start 1024 largefile.dat

# View with 32 bytes per line starting from hex offset 0x200
hexv --start 0x200 --bytes-per-line 32 firmware.bin
```

## Dependencies

- **clap**: Command-line argument parsing
- **crossterm**: Cross-platform terminal manipulation
- **ratatui**: Terminal UI framework
- **tokio**: Asynchronous runtime

## Building

Requirements:
- Rust 2024 edition
- Cargo

```bash
# Debug build
cargo build

# Release build
cargo build --release

```


## License

This project is licensed under the MIT License - see the LICENSE file for details.
