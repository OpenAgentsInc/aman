# signal-cli Daemon Guide

This guide covers running signal-cli in daemon mode for the Aman project, including startup configuration, JSON-RPC and D-Bus interfaces, and subscribing to incoming message events.

## Building signal-cli

From the repository root:

```bash
cd repos/signal-cli
./gradlew installDist
```

The binary will be at `repos/signal-cli/build/install/signal-cli/bin/signal-cli`.

## Daemon Modes

signal-cli daemon supports four transport mechanisms that can be enabled simultaneously:

| Transport | Flag | Default Endpoint |
|-----------|------|------------------|
| UNIX Socket | `--socket[=PATH]` | `$XDG_RUNTIME_DIR/signal-cli/socket` |
| TCP Socket | `--tcp[=HOST:PORT]` | `localhost:7583` |
| HTTP | `--http[=HOST:PORT]` | `localhost:8080` |
| D-Bus | `--dbus` / `--dbus-system` | Session or system bus |

### Basic Usage

```bash
# Single account with HTTP
signal-cli -a +1234567890 daemon --http=0.0.0.0:8080

# Single account with multiple transports
signal-cli -a +1234567890 daemon --http --socket --dbus

# Multi-account mode (no -a flag)
signal-cli daemon --http --socket
```

---

## Startup Scripts

### Option 1: Simple Shell Script

Create `scripts/start-signal-daemon.sh`:

```bash
#!/bin/bash
set -e

SIGNAL_CLI="${SIGNAL_CLI_PATH:-./repos/signal-cli/build/install/signal-cli/bin/signal-cli}"
ACCOUNT="${AMAN_NUMBER:?AMAN_NUMBER must be set}"
CONFIG_DIR="${SIGNAL_CLI_CONFIG:-$HOME/.local/share/signal-cli}"

exec "$SIGNAL_CLI" \
    --config "$CONFIG_DIR" \
    -a "$ACCOUNT" \
    daemon \
    --http=127.0.0.1:8080 \
    --receive-mode=on-start \
    --send-read-receipts
```

### Option 2: Systemd User Service

Create `~/.config/systemd/user/signal-cli.service`:

```ini
[Unit]
Description=signal-cli daemon for Aman
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
Environment="SIGNAL_CLI_OPTS=-Xms2m"
ExecStart=/path/to/signal-cli -a +1234567890 daemon --http=127.0.0.1:8080
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user daemon-reload
systemctl --user enable signal-cli
systemctl --user start signal-cli
```

### Option 3: Systemd System Service with Socket Activation

For production deployments, use socket activation with security hardening. Copy from `repos/signal-cli/data/`:

1. Create system user:
   ```bash
   useradd -r -U -s /usr/sbin/nologin -m -b /var/lib signal-cli
   ```

2. Install service files:
   ```bash
   sudo cp repos/signal-cli/data/signal-cli-socket.socket /etc/systemd/system/
   sudo cp repos/signal-cli/data/signal-cli-socket.service /etc/systemd/system/
   ```

3. Edit `/etc/systemd/system/signal-cli-socket.service` to set the correct path to signal-cli binary.

4. Enable:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable signal-cli-socket.socket
   sudo systemctl start signal-cli-socket.socket
   ```

---

## JSON-RPC Interface

### HTTP Endpoints

When running with `--http`, three endpoints are exposed:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/v1/rpc` | POST | JSON-RPC 2.0 commands |
| `/api/v1/events` | GET | Server-Sent Events stream |
| `/api/v1/check` | GET | Health check (returns 200 OK) |

### Request Format

```json
{"jsonrpc": "2.0", "method": "methodName", "params": {...}, "id": "unique-id"}
```

### Sending Messages

```bash
# Send to individual
curl -X POST http://localhost:8080/api/v1/rpc \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "send",
    "params": {
      "recipient": ["+9876543210"],
      "message": "Hello from Aman"
    },
    "id": "1"
  }'

# Send to group
curl -X POST http://localhost:8080/api/v1/rpc \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "send",
    "params": {
      "groupId": "GROUP_ID_BASE64",
      "message": "Hello group"
    },
    "id": "2"
  }'
```

### Common Methods

| Method | Description |
|--------|-------------|
| `send` | Send message to recipient(s) or group |
| `listGroups` | List all groups |
| `listContacts` | List contacts |
| `getSelfNumber` | Get bot's phone number |
| `listDevices` | List linked devices |
| `updateProfile` | Update profile name/avatar |
| `sendTyping` | Send typing indicator (start/stop) |
| `subscribeReceive` | Manual subscription for messages |
| `unsubscribeReceive` | Stop a subscription |

### Parameter Mapping

CLI parameters become camelCase in JSON-RPC:
- `--group-id=ID` → `"groupId": "ID"`
- `--attachment FILE1 FILE2` → `"attachments": ["FILE1", "FILE2"]`
- `--message TEXT` → `"message": "TEXT"`

### Multi-Account Mode

When daemon runs without `-a`, include account in params:

```json
{
  "jsonrpc": "2.0",
  "method": "send",
  "params": {
    "account": "+1234567890",
    "recipient": ["+9876543210"],
    "message": "Hello"
  },
  "id": "1"
}
```

---

## D-Bus Interface

### Starting with D-Bus

```bash
# Session bus (user-level)
signal-cli -a +1234567890 daemon --dbus

# System bus (requires policy config)
sudo signal-cli -a +1234567890 daemon --dbus-system
```

### D-Bus Policy Configuration

For system bus, install `/etc/dbus-1/system.d/org.asamk.Signal.conf`:

```xml
<?xml version="1.0"?>
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
        "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
    <policy user="signal-cli">
        <allow own="org.asamk.Signal"/>
        <allow send_destination="org.asamk.Signal"/>
        <allow receive_sender="org.asamk.Signal"/>
    </policy>
    <policy context="default">
        <allow send_destination="org.asamk.Signal"/>
        <allow receive_sender="org.asamk.Signal"/>
    </policy>
</busconfig>
```

### Object Paths

- Single-account mode: `/org/asamk/Signal`
- Multi-account mode: `/org/asamk/Signal/_<phonenumber>` (+ replaced with _)
  - Example: `/org/asamk/Signal/_1234567890`

### Sending Messages via D-Bus

```bash
# Send text message
dbus-send --session --print-reply --type=method_call \
  --dest="org.asamk.Signal" /org/asamk/Signal \
  org.asamk.Signal.sendMessage \
  string:"Hello" array:string: string:+9876543210

# Send group message (groupId as byte array)
dbus-send --session --print-reply --type=method_call \
  --dest=org.asamk.Signal /org/asamk/Signal \
  org.asamk.Signal.sendGroupMessage \
  string:"Hello group" array:string: \
  array:byte:139,22,72,247,116,32,170,104,205,164,207,21,248,77,185

# List groups
dbus-send --session --print-reply --type=method_call \
  --dest=org.asamk.Signal /org/asamk/Signal \
  org.asamk.Signal.listGroups
```

### Key D-Bus Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `sendMessage` | `(s, as, s) → x` | Send message with attachments to recipient |
| `sendGroupMessage` | `(s, as, ay) → x` | Send to group |
| `listGroups` | `() → a(oays)` | List groups |
| `getContactName` | `(s) → s` | Get contact name by number |
| `getSelfNumber` | `() → s` | Get bot's number |
| `version` | `() → s` | Get signal-cli version |

---

## Subscribing to Incoming Events

### Method 1: HTTP Server-Sent Events (Recommended for Aman)

The SSE endpoint provides a continuous stream of incoming messages:

```bash
# Subscribe to events
curl -N "http://localhost:8080/api/v1/events"

# Multi-account mode (URL-encode the +)
curl -N "http://localhost:8080/api/v1/events?account=%2B1234567890"
```

Event format:

```
event: receive
data: {"envelope":{"source":"+9876543210","sourceNumber":"+9876543210","sourceUuid":"uuid","sourceName":"Contact Name","sourceDevice":1,"timestamp":1631458508784,"dataMessage":{"timestamp":1631458508784,"message":"Hello","expiresInSeconds":0,"viewOnce":false}}}
```

#### JavaScript/Node.js Client

```javascript
const EventSource = require('eventsource');

const es = new EventSource('http://localhost:8080/api/v1/events');

es.addEventListener('receive', (event) => {
  const data = JSON.parse(event.data);
  const envelope = data.envelope;

  if (envelope.dataMessage?.message) {
    console.log(`From: ${envelope.source}`);
    console.log(`Message: ${envelope.dataMessage.message}`);
  }
});

es.onerror = (err) => {
  console.error('SSE Error:', err);
};
```

#### Rust Client (using reqwest + eventsource)

```rust
use eventsource_client as es;
use futures::StreamExt;

async fn subscribe_to_messages() -> Result<(), Box<dyn std::error::Error>> {
    let client = es::ClientBuilder::for_url("http://localhost:8080/api/v1/events")?
        .build();

    let mut stream = client.stream();

    while let Some(event) = stream.next().await {
        match event {
            Ok(es::SSE::Event(ev)) if ev.event_type == "receive" => {
                println!("Received: {}", ev.data);
            }
            Ok(_) => {}
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    Ok(())
}
```

### Method 2: JSON-RPC Automatic Receive (Default)

With `--receive-mode=on-start` (default), messages arrive as JSON-RPC notifications on socket/TCP connections:

```json
{"jsonrpc":"2.0","method":"receive","params":{"envelope":{"source":"+9876543210",...}}}
```

### Method 3: JSON-RPC Manual Subscribe

For finer control, use `--receive-mode=manual`:

```bash
signal-cli -a +1234567890 daemon --socket --receive-mode=manual
```

Then explicitly subscribe:

```json
{"jsonrpc":"2.0","method":"subscribeReceive","id":"1"}
```

Response:

```json
{"jsonrpc":"2.0","result":0,"id":"1"}
```

Messages arrive wrapped with subscription ID:

```json
{"jsonrpc":"2.0","method":"receive","params":{"subscription":0,"result":{"envelope":{...}}}}
```

To unsubscribe:

```json
{"jsonrpc":"2.0","method":"unsubscribeReceive","params":{"subscription":0},"id":"2"}
```

### Method 4: D-Bus Signals

Subscribe to D-Bus signals for incoming messages:

```bash
# Monitor all Signal events
dbus-monitor --session "type='signal',interface='org.asamk.Signal'"
```

Available signals:
- `MessageReceived(timestamp, sender, groupId, message, attachments)` - Incoming message
- `ReceiptReceived(timestamp, sender)` - Delivery receipt
- `SyncMessageReceived(timestamp, sender, destination, groupId, message, attachments)` - Sync from linked device

#### Python D-Bus Client Example

```python
from gi.repository import GLib
import dbus
from dbus.mainloop.glib import DBusGMainLoop

def message_received(timestamp, sender, groupId, message, attachments):
    print(f"From: {sender}")
    print(f"Message: {message}")

DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()

bus.add_signal_receiver(
    message_received,
    dbus_interface="org.asamk.Signal",
    signal_name="MessageReceived"
)

loop = GLib.MainLoop()
loop.run()
```

---

## Daemon Options Reference

```
signal-cli [GLOBAL] daemon [OPTIONS]

GLOBAL OPTIONS:
  --verbose, -v              Increase log verbosity
  --config CONFIG            Config directory (default: ~/.local/share/signal-cli)
  -a ACCOUNT                 Account phone number (omit for multi-account)
  --trust-new-identities     {on-first-use|always|never}
  --scrub-log                Scrub sensitive data from logs

DAEMON OPTIONS:
  --socket[=PATH]            Enable UNIX socket
  --tcp[=HOST:PORT]          Enable TCP socket (default: localhost:7583)
  --http[=HOST:PORT]         Enable HTTP server (default: localhost:8080)
  --dbus                     Enable D-Bus session bus
  --dbus-system              Enable D-Bus system bus
  --bus-name NAME            Custom D-Bus bus name
  --receive-mode MODE        on-start (default) or manual
  --no-receive-stdout        Don't print messages to stdout
  --ignore-attachments       Skip attachment downloads
  --ignore-stories           Don't receive story messages
  --send-read-receipts       Auto-send read receipts
```

---

## Troubleshooting

### Check daemon is running

```bash
curl http://localhost:8080/api/v1/check
# Returns 200 OK if running
```

### View daemon logs

```bash
# If running via systemd
journalctl --user -u signal-cli -f

# Or add --verbose flag
signal-cli --verbose -a +1234567890 daemon --http
```

### Test JSON-RPC connectivity

```bash
curl -X POST http://localhost:8080/api/v1/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"version","id":"test"}'
```

### Common issues

1. **"Account not found"**: Run `signal-cli -a +NUMBER register` first
2. **"Untrusted identity"**: Use `--trust-new-identities=on-first-use`
3. **Permission denied on socket**: Check socket file permissions and group membership
4. **D-Bus connection refused**: Ensure D-Bus policy file is installed for system bus
