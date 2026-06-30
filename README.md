# MeshNet

> Fast, zero-config local file transfer for macOS, Linux & Android (Termux)

MeshNet lets you send files between devices on the same Wi-Fi network — no cloud, no accounts, no configuration needed. It auto-discovers peers using mDNS and a direct subnet scan, so it works even on Android where mDNS is typically blocked.

---

## Features

| | |
|---|---|
| ⚡ **Blazing fast** | Streams directly over TCP using async Rust |
| 🔍 **Auto-discovery** | Finds peers via mDNS + subnet scan simultaneously |
| 📱 **Android-ready** | Works on Termux without any special Android permissions |
| 📝 **Direct IP fallback** | Type an IP manually if auto-discovery fails |
| 🔁 **Two-way** | Every node sends and receives at the same time |
| 🖥️ **Cross-platform** | macOS · Linux · Android (Termux) |

---

## Quick Start

```
┌──────────── Device A (receiver) ────────────┐
│  meshnet                                     │
│  > Receiver is now active                   │
│  > Address  192.168.1.7:7878                │
└─────────────────────────────────────────────┘

┌──────────── Device B (sender) ──────────────┐
│  meshnet send                               │
│  > Scanning for devices...                  │
│  > Select: Device A [macos] 192.168.1.7     │
│  > File: ~/Desktop/video.mp4               │
│  > ████████████ 45.2 MB  done ✓            │
└─────────────────────────────────────────────┘
```

---

## Installation

### Prerequisites — install Rust

| Platform | Command |
|---|---|
| macOS / Linux | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Termux (Android) | `pkg update && pkg install git rust` |

### Install MeshNet

```bash
git clone https://github.com/sameer92921/meshnet.git
cd meshnet
cargo build --release
```

**macOS / Linux** — install globally:
```bash
sudo cp target/release/meshnet /usr/local/bin/
```

**Termux (Android)** — install globally:
```bash
cp target/release/meshnet $PREFIX/bin/
```

---

## Usage

### Interactive mode (recommended)

Just run `meshnet` with no arguments. It starts the receiver in the background and gives you a menu:

```bash
meshnet
```

```
  What would you like to do?
  > 📤  Send a file
    🔍  Scan for devices
    ❌  Exit
```

When sending, you will be shown a list of discovered devices. If your device isn't listed, choose **"Enter IP manually"** and type the address shown on the other device (e.g. `192.168.1.4:7878`).

---

### CLI commands

#### `meshnet serve [PATH]`
Start the file-receiver daemon. Saves incoming files to `PATH` (defaults to current directory).

```bash
meshnet serve ~/Downloads
```

#### `meshnet list`
Scan the network and print all active MeshNet devices.

```bash
meshnet list
```

#### `meshnet send`
Send a file interactively. Scans for devices and lets you pick one.

```bash
meshnet send
```

#### `meshnet send --file FILE --ip IP:PORT`
Send a file directly to a known IP, bypassing discovery.

```bash
meshnet send --file ~/Desktop/report.pdf --ip 192.168.1.4:7878
```

#### `meshnet send --file FILE --device NAME`
Send to a device matched by name (partial match, case-insensitive).

```bash
meshnet send --file ./photo.jpg --device "pixel"
```

---

## Troubleshooting

### Device not showing up in scan?

1. **Check both devices are on the same Wi-Fi** (not one on 2.4 GHz and another on 5 GHz if the router isolates them).
2. **Android — disable VPNs** (e.g. AdGuard, Private DNS, WireGuard) before running meshnet. They block incoming TCP connections.
3. **Use direct IP** — run `meshnet` on the receiver and note the `Address` line. Enter it manually with `Enter IP manually` in the sender's menu, or:
   ```bash
   meshnet send --ip 192.168.1.4:7878
   ```

### `~/path/to/file` not found?

MeshNet expands `~` automatically. Make sure there is no extra space before the path and the file actually exists.

---

## Updating

Pull the latest changes and rebuild. Run these commands inside the cloned `meshnet` folder.

**macOS / Linux:**
```bash
git pull origin main
cargo build --release
sudo cp target/release/meshnet /usr/local/bin/
```

**Termux (Android):**
```bash
git pull origin main
cargo build --release
cp target/release/meshnet $PREFIX/bin/
```

---

## License

MIT
