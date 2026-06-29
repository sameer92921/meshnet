# MeshNet

MeshNet is a blazing-fast, cross-platform command-line tool built with Rust that allows you to send files locally between connected devices (macOS, Linux, and Android via Termux) over your Wi-Fi network without requiring any central servers.

It uses mDNS (ZeroConf/Bonjour) to automatically discover other MeshNet devices on your local network. It also supports direct IP entry if Android/routers block multicast discovery packets.

## Features

- **Two-Way Communication**: By default, MeshNet runs in a unified mode. It listens for incoming files in the background while letting you send files interactively in the foreground.
- **Blazing Fast**: Written in Rust, it streams files directly to disk using `tokio` and `axum`.
- **Interactive Prompts**: Easily select the file you want to send and the destination device from a visual menu.
- **Cross-Platform**: Compiles to a static binary that runs seamlessly on macOS, Linux, and Android (Termux).
- **Direct IP Fallback**: Android devices sometimes block mDNS discovery. You can easily bypass this by typing the IP address directly.

---

## Setup Instructions

### Prerequisites
You need to have Rust and Cargo installed on your system.
If you don't have it installed, run:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 1. macOS & Linux Setup
1. Clone the repository:
   ```bash
   git clone https://github.com/sameer92921/meshnet.git
   cd meshnet
   ```
2. Build the optimized release binary:
   ```bash
   cargo build --release
   ```
3. (Optional) Move the binary to your path to use it globally:
   ```bash
   sudo mv target/release/meshnet /usr/local/bin/
   ```

### 2. Android (Termux) Setup
1. Download and install [Termux](https://f-droid.org/en/packages/com.termux/) from F-Droid.
2. Update packages and install Rust and Git:
   ```bash
   pkg update && pkg upgrade
   pkg install git rust
   ```
3. Clone the repository and build:
   ```bash
   git clone https://github.com/sameer92921/meshnet.git
   cd meshnet
   cargo build --release
   ```
4. Copy the binary to your Termux bin folder:
   ```bash
   cp target/release/meshnet $PREFIX/bin/
   ```

---

## Usage Guide

### Two-Way Interactive Mode (Recommended)
Simply run the command with no arguments:
```bash
meshnet
```
This automatically starts the receiver daemon in the background (printing your device's IP and Port) and opens an interactive prompt. From there, you can choose to send files, list devices, or manually enter an IP address to send a file to a device that mDNS couldn't discover (e.g., your Termux app).

### Manual CLI Commands
If you prefer scripting or using manual arguments:

**1. Start the Receiver Daemon (Blocking)**
```bash
meshnet serve ~/Downloads/meshnet_files
```

**2. Send a File to a Discovered Device**
```bash
meshnet send --file ./my_video.mp4 --device "Sameer-MacBook"
```

**3. Send a File via Direct IP (Bypassing Discovery)**
```bash
meshnet send --file ./my_video.mp4 --ip 192.168.1.15:8080
```

---

## Updating to the Latest Version

If you installed MeshNet by cloning from GitHub, you can easily update it to get new features and bug fixes.

**On macOS / Linux:**
```bash
cd meshnet
git pull origin main
cargo build --release
sudo mv target/release/meshnet /usr/local/bin/
```

**On Android (Termux):**
```bash
cd meshnet
git pull origin main
cargo build --release
cp target/release/meshnet $PREFIX/bin/
```
