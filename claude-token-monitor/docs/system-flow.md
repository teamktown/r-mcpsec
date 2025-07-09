# Claude Token Monitor - System Flow (Passive Monitoring)

## Application Data Flow

```mermaid
flowchart TD
    A[Application Start] --> B[Parse CLI Arguments]
    B --> C{--force-mock flag?}
    C -->|Yes| D[Initialize Mock TokenMonitor]
    C -->|No| E[Initialize FileBasedTokenMonitor]
    
    E --> F[Discover Claude Data Paths]
    F --> G[~/.claude/projects/**/*.jsonl]
    F --> H[~/.config/claude/projects/**/*.jsonl]
    F --> I[CLAUDE_DATA_PATHS env var]
    
    G --> J[Validate and Canonicalize Paths]
    H --> J
    I --> J
    
    J --> K[Initialize SessionTracker for Observation]
    K --> L[Load Data Directory]
    L --> M[Create ~/.local/share/claude-token-monitor/]
    M --> N[Load observed_sessions.json]
    N --> O[Load config.json]
    O --> P[Start Command Processing]
    
    P --> Q{Command Type}
    Q -->|monitor| R[Start Passive Monitoring]
    Q -->|status| S[Show Observed Session Status]
    Q -->|history| T[Show Observed Session History]
    Q -->|config| U[Update Config]
    
    R --> V[Scan Usage Files]
    V --> W[FileBasedTokenMonitor.scan_usage_files()]
    W --> X[Find All .jsonl Files]
    X --> Y[Parse JSONL Entries]
    
    Y --> Z[Validate JSON with Size/Depth Limits]
    Z --> AA[Extract Usage Data]
    AA --> BB[Filter Valid Usage Entries]
    BB --> CC[Sort by Timestamp]
    CC --> DD[Deduplicate by Message/Request ID]
    
    DD --> EE[Derive Current Session]
    EE --> FF[FileBasedTokenMonitor.derive_current_session()]
    FF --> GG[Find Most Recent Entry]
    GG --> HH[Calculate 5-Hour Session Window]
    HH --> II[Determine Session Activity]
    II --> JJ[Generate Deterministic Session ID]
    JJ --> KK[Calculate Total Tokens in Session]
    
    KK --> LL[Calculate Metrics]
    LL --> MM[FileBasedTokenMonitor.calculate_metrics()]
    MM --> NN[Usage Rate = tokens/time_elapsed]
    NN --> OO[Session Progress = elapsed/duration]
    OO --> PP[Efficiency = expected/actual rate]
    PP --> QQ[Projected Depletion = remaining/rate]
    
    QQ --> RR[Update UI]
    RR --> SS{UI Type}
    SS -->|--basic-ui| TT[TerminalUI.display]
    SS -->|default| UU[RatatuiTerminalUI.render]
    
    UU --> VV{Selected Tab}
    VV -->|0| WW[Overview: Observed Session & Stats]
    VV -->|1| XX[Charts: Usage Distribution]
    VV -->|2| YY[Session: Observed Details & Predictions]
    VV -->|3| ZZ[Details: File Sources & Token Analysis]
    VV -->|4| AAA[Security: Analysis Summary]
    VV -->|5| BBB[Settings: Config & Technical Info]
    VV -->|6| CCC[About: Attribution & Version]
    
    WW --> DDD[Sleep Update Interval]
    XX --> DDD
    YY --> DDD
    ZZ --> DDD
    AAA --> DDD
    BBB --> DDD
    CCC --> DDD
    TT --> DDD
    
    DDD --> EEE{Continue Monitoring?}
    EEE -->|Yes| V
    EEE -->|No| FFF[Stop Monitoring]
    FFF --> GGG[Save Final State]
    GGG --> HHH[Exit Application]
    
    style A fill:#e1f5fe
    style E fill:#e8f5e8
    style D fill:#fff3e0
    style V fill:#e3f2fd
    style UU fill:#f3e5f5
    style HHH fill:#e0f2f1
```

## Passive File Operations Flow

```mermaid
flowchart LR
    A[Claude Code Usage] --> B[~/.claude/projects/**/*.jsonl]
    B --> C[FileBasedTokenMonitor.scan_usage_files()]
    C --> D[Parse JSONL Lines]
    D --> E[Extract Usage Data]
    E --> F[TokenUsage Structure]
    
    F --> G[input_tokens]
    F --> H[output_tokens]
    F --> I[cache_creation_input_tokens]
    F --> J[cache_read_input_tokens]
    
    G --> K[Calculate Total Tokens]
    H --> K
    I --> K
    J --> K
    
    K --> L[Group by Time Windows]
    L --> M[Derive Session Information]
    M --> N[PASSIVE Session Creation]
    
    N --> O[observed-{timestamp}]
    O --> P[Calculate Session Metrics]
    P --> Q[Save to observed_sessions.json]
    
    R[Session Data] --> S[~/.local/share/claude-token-monitor/]
    S --> T[observed_sessions.json]
    S --> U[config.json]
    
    style B fill:#e3f2fd
    style C fill:#e8f5e8
    style N fill:#fff3e0
    style T fill:#fce4ec
    style U fill:#fff8e1
```

## Passive Monitoring Architecture

### Key Principles
1. **No Session Creation**: Tool only observes existing sessions from JSONL data
2. **Passive Observation**: Reads Claude Code's usage files without modification
3. **Derived Sessions**: Sessions are calculated from usage patterns, not managed
4. **File-Based**: No API calls or authentication required
5. **Real-time Updates**: File system watching for immediate updates

### Data Sources
- **Primary**: `~/.claude/projects/**/*.jsonl` files written by Claude Code
- **Secondary**: `~/.config/claude/projects/**/*.jsonl` (alternative location)
- **Environment**: `CLAUDE_DATA_PATHS` and `CLAUDE_DATA_PATH` variables

### Session Derivation Logic
```
1. Parse all JSONL entries from usage files
2. Sort by timestamp chronologically
3. Find most recent entry to determine current session
4. Calculate 5-hour session window from most recent entry
5. Sum all tokens within that window
6. Generate deterministic session ID: "observed-{timestamp}"
7. Determine if session is active based on current time vs reset time
```

## Calculation Details

### Usage Rate Calculation (Passive)
```
usage_rate = total_session_tokens / time_elapsed_minutes
where:
  total_session_tokens = sum of all tokens in current session window
  time_elapsed_minutes = minutes since session start
```

### Efficiency Score Calculation
```
expected_rate = token_limit / session_duration_minutes
actual_rate = current_usage_rate
efficiency = min(1.0, max(0.0, expected_rate / actual_rate))
```

### Projected Depletion
```
remaining_tokens = token_limit - current_tokens
time_to_depletion = remaining_tokens / usage_rate
depletion_time = current_time + time_to_depletion
```

### Session Progress
```
elapsed_time = current_time - session_start_time
session_duration = 5 hours (18000 seconds)
progress = elapsed_time / session_duration
```

## Data Persistence

### Observed Session Storage (observed_sessions.json)
```json
[
  {
    "id": "observed-1752068062",
    "plan_type": "Max20",
    "tokens_used": 54143,
    "tokens_limit": 100000,
    "start_time": "2025-07-09T13:34:39Z",
    "reset_time": "2025-07-09T18:34:39Z",
    "is_active": true,
    "end_time": null
  }
]
```

### Configuration (config.json)
```json
{
  "default_plan": "Pro",
  "timezone": "UTC",
  "update_interval_seconds": 3,
  "warning_threshold": 0.85,
  "auto_switch_plans": true,
  "color_scheme": {
    "progress_bar_full": "green",
    "progress_bar_empty": "gray",
    "warning_color": "yellow",
    "success_color": "green",
    "error_color": "red",
    "info_color": "blue"
  }
}
```

### Claude Code JSONL Format (Read-Only)
```json
{
  "timestamp": "2025-07-09T13:34:39.652Z",
  "message": {
    "id": "msg_123",
    "model": "claude-sonnet-4-20250514",
    "usage": {
      "input_tokens": 150,
      "output_tokens": 287,
      "cache_creation_input_tokens": 0,
      "cache_read_input_tokens": 0
    }
  },
  "requestId": "req_456"
}
```

## Security Features

### Input Validation
- **JSON Size Limits**: 1MB max per JSON line
- **Depth Limits**: 32 levels maximum nesting
- **File Size Limits**: 50MB max file size
- **Path Validation**: Canonicalization to prevent traversal attacks

### Safe File Operations
- **Read-Only**: Never writes to Claude Code files
- **Path Sanitization**: Validates all file paths before access
- **Error Handling**: Graceful handling of malformed data
- **Memory Safety**: Rust ownership prevents buffer overflows

### Privacy Protection
- **No API Access**: No network connections to Claude servers
- **Local Processing**: All data processing happens locally
- **Content Agnostic**: Only reads token counts, not conversation content
- **Minimal Logging**: Sensitive data redacted in debug output