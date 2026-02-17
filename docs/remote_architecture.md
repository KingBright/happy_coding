# Happy Remote Architecture

## Overview

Happy Remote is designed for 1-to-many session management with a focus on zero external dependencies and real-time visibility.

## Key Components

### 1. Native Persistence Layer (persistence.rs)

**Goal**: Provide tmux-like functionality without external tools.

**Implementation**:
- Uses `portable-pty` for cross-platform PTY management
- 10MB ring buffer per session for scrollback history
- JSON state files for session recovery
- Fork-based daemonization (Unix) / service-based (Windows)

**Key Structures**:
```rust
PersistentSession {
    cmd_tx: mpsc::Sender<Vec<u8>>,      // Input channel
    output_tx: broadcast::Sender<Bytes>, // Output broadcast
    buffer: Arc<RwLock<RingBuffer>>,     // 10MB scrollback
    metadata: SessionMetadata,           // Persistable state
}
```

### 2. Session Multiplexer (multiplexer.rs)

**Goal**: Manage multiple sessions, enable client attachment.

**Features**:
- Create/kill/attach to sessions
- Client connection tracking
- Output broadcasting to multiple clients

### 3. Error Handling & Recovery (error.rs)

**Goal**: Graceful degradation and visibility.

**Components**:
- Circuit breaker pattern for external services
- Exponential backoff retry
- Error aggregation for dashboard
- Automatic recovery strategies

### 4. Metrics Collection (metrics.rs)

**Goal**: Real-time visibility into all sessions.

**Metrics Tracked**:
- I/O bytes (in/out)
- Client connection count
- Session status (Initializing/Running/Idle/Error)
- Progress percentage
- Confirmation requirements
- Error counts

### 5. Global Dashboard (dashboard.rs)

**Goal**: 1-to-many session management.

**Features**:
- Summary cards (total, running, waiting, errors)
- Real-time session grid
- Bulk actions (select multiple, confirm all, kill selected)
- Progress visualization
- Confirmation dialog proxy
- Filtering and sorting

## Data Flow

### Session Creation
```
CLI: happy run claude
  └─► Daemon::create_session()
      ├─► PTY pair created
      ├─► Child process spawned (claude)
      ├─► I/O tasks started
      ├─► State saved to disk
      └─► Metrics registered
```

### Browser Connection
```
Browser WebSocket connect
  └─► DaemonServer::handle_connection()
      ├─► ClientMessage::ListSessions
      │   └─► Returns all session summaries
      ├─► ClientMessage::AttachSession
      │   └─► Subscribe to output broadcast
      │   └─► Send initial buffer contents
      └─► Real-time output forwarding
```

### Confirmation Handling
```
Claude outputs: "Do you want to continue? (y/n)"
  └─► PTY reader detects prompt pattern
  └─► Metrics::set_confirmation_state(true, prompt)
  └─► Dashboard shows confirmation box
  └─► User clicks "Confirm" in browser
  └─► Daemon sends "y\n" to PTY
  └─► Confirmation state cleared
```

## Security

### Encryption
- X25519 key exchange
- XSalsa20-Poly1305 authenticated encryption
- Per-session ephemeral keys

### Authentication
- JWT for session tokens
- API keys for machine registration
- Argon2 for password hashing

## Recovery

### Daemon Restart
1. Scan state directory for session files
2. Check if PIDs are still running
3. Mark running sessions as "detached"
4. User can re-attach via browser

### Network Disconnect
1. Session continues running
2. Buffer accumulates output
3. Client reconnects → receives buffer + live updates

## Scalability

### Single Machine Limits
- Sessions: Limited by PTY count (typically 1024)
- Memory: ~10MB per session (ring buffer)
- CPU: Depends on Claude workload

### Optimization Strategies
- Buffer size configurable per session
- Automatic cleanup of exited sessions
- Metrics aggregation with sampling

## Future Enhancements

### Planned
- [ ] Distributed daemon (multiple machines)
- [ ] Recording/playback
- [ ] AI-assisted session management
- [ ] Mobile app (Tauri/React Native)

### Considered
- - Kubernetes operator
- Cloud-hosted option
- Team collaboration features
