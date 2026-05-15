## ThreatDeck Release

### Binaries

| Platform | Architecture | Artifact |
|----------|--------------|----------|
| Linux | x86_64 | `ThreatDeck-x86_64-linux` |
| macOS | x86_64 | `ThreatDeck-x86_64-macos` |
| macOS | Apple Silicon / ARM64 | `ThreatDeck-aarch64-macos` |

### Installation

Download the binary for your platform, make it executable, and place it in your `$PATH`:

```bash
chmod +x ThreatDeck-<platform>
sudo mv ThreatDeck-<platform> /usr/local/bin/ThreatDeck
```

### Verification

Verify the checksum:

```bash
sha256sum -c SHA256SUMS
```
