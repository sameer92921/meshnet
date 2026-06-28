# MeshNet

MeshNet is a blazing-fast, cross-platform command-line tool built with Rust that allows you to send files locally between connected devices (macOS, Linux, and Android via Termux) over your Wi-Fi network without requiring any central servers.

It uses mDNS (ZeroConf/Bonjour) to automatically discover other MeshNet devices on your local network.

## Features

- **Blazing Fast**: Written in Rust, it streams files directly to disk using `tokio` and `axum`.
- **Interactive Prompts**: Easily select the file you want to send and the destination device from a visual menu.
- **Cross-Platform**: Compiles to a static binary that runs seamlessly on macOS, Linux, and Android (Termux).
- **Zero Config**: No need to type IP addresses, devices discover each other automatically.

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

### 1. Start the Receiver Daemon
To receive files, you need to start the MeshNet daemon on the destination device.

```bash
meshnet serve
```
*(If you are running it with cargo, use `cargo run --release -- serve`)*

By default, files will be saved in the directory where you run the command. You can also specify a custom download path:
```bash
meshnet serve ~/Downloads/meshnet_files
```

### 2. List Devices (Optional)
To see a list of active MeshNet devices on your current Wi-Fi network:
```bash
meshnet list
```

### 3. Send a File (Interactive Mode)
To send a file, simply run the send command on the sending device:
```bash
meshnet send
```
You will be greeted with an interactive prompt to:
1. Type the path to the file you want to send.
2. Select a discovered device from a visual menu.

### 4. Send a File (Fast/CLI Mode)
If you want to skip the interactive prompts, you can pass the file path and device name directly as arguments:
```bash
meshnet send --file ./my_video.mp4 --device "Sameer-MacBook"
```
