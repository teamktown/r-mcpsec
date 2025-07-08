# Claude Token Monitor - System Flow

## Application Data Flow

```mermaid
flowchart TD
    A[Application Start] --> B[Parse CLI Arguments]
    B --> C{--force-mock flag?}
    C -->|Yes| D[Initialize Mock TokenMonitor]
    C -->|No| E[Check Credentials]
    
    E --> F[Try CLI Args --api-key]
    F --> G{API Key Found?}
    G -->|Yes| H[Create ApiClient]
    G -->|No| I[Try ~/.claude/.credentials.json]
    I --> J{Credentials Found?}
    J -->|Yes| H
    J -->|No| K[Try Environment Variables]
    K --> L{CLAUDE_API_KEY set?}
    L -->|Yes| H
    L -->|No| M[❌ Exit with Error]
    
    H --> N[Test API Connection]
    N --> O{Connection OK?}
    O -->|Yes| P[Create Real TokenMonitor]
    O -->|No| M
    
    D --> Q[Load Data Directory]
    P --> Q
    Q --> R[Create ~/.local/share/claude-token-monitor/]
    R --> S[Load sessions.json]
    S --> T[Load config.json]
    T --> U[Initialize SessionTracker]
    U --> V[Start Command Processing]
    
    V --> W{Command Type}
    W -->|monitor| X[Start Monitoring]
    W -->|status| Y[Show Session Status]
    W -->|create| Z[Create New Session]
    W -->|end| AA[End Session]
    W -->|history| BB[Show History]
    W -->|config| CC[Update Config]
    W -->|auth| DD[Auth Commands]
    
    X --> EE[Ensure Active Session]
    EE --> FF{Session Exists?}
    FF -->|No| GG[Create New Session]
    FF -->|Yes| HH[Use Existing Session]
    GG --> HH
    HH --> II[Start Background Monitoring Loop]
    
    II --> JJ[TokenMonitor.monitoring_loop]
    JJ --> KK[Set update interval timer]
    KK --> LL[Fetch Current Usage]
    
    LL --> MM{Mock Mode?}
    MM -->|Yes| NN[Generate Random Mock Data]
    MM -->|No| OO[Call Claude API]
    
    NN --> PP[Return Simulated Usage]
    OO --> QQ{API Success?}
    QQ -->|Yes| RR[Return Real Usage Data]
    QQ -->|No| SS[❌ Log Error & Continue]
    
    PP --> TT[Update Session Data]
    RR --> TT
    SS --> TT
    
    TT --> UU[Calculate Metrics]
    UU --> VV[Usage Rate = tokens/time]
    VV --> WW[Efficiency = expected/actual rate]
    WW --> XX[Session Progress = elapsed/duration]
    XX --> YY[Projected Depletion = remaining/rate]
    
    YY --> ZZ[Update UsageMetrics]
    ZZ --> AAA[Save Session to JSON]
    AAA --> BBB[Update UI]
    
    BBB --> CCC{UI Type}
    CCC -->|--basic-ui| DDD[TerminalUI.display]
    CCC -->|default| EEE[RatatuiTerminalUI.render]
    
    EEE --> FFF{Selected Tab}
    FFF -->|0| GGG[Overview: Gauges & Stats]
    FFF -->|1| HHH[Charts: Bar Charts]
    FFF -->|2| III[Session: Details & Predictions]
    FFF -->|3| JJJ[Settings: Config & Technical Info]
    
    GGG --> KKK[Sleep Update Interval]
    HHH --> KKK
    III --> KKK
    JJJ --> KKK
    DDD --> KKK
    
    KKK --> LLL{Continue Monitoring?}
    LLL -->|Yes| LL
    LLL -->|No| MMM[Stop Monitoring]
    MMM --> NNN[Save Final State]
    NNN --> OOO[Exit Application]
    
    style A fill:#e1f5fe
    style M fill:#ffebee
    style P fill:#e8f5e8
    style D fill:#fff3e0
    style EEE fill:#f3e5f5
    style OOO fill:#e0f2f1
```

## File Operations Flow

```mermaid
flowchart LR
    A[Session Data] --> B[~/.local/share/claude-token-monitor/]
    B --> C[sessions.json]
    B --> D[config.json]
    
    E[Credentials] --> F[CLI --api-key]
    E --> G[~/.claude/.credentials.json]
    E --> H[CLAUDE_API_KEY env var]
    
    I[API Client] --> J[fetch_token_usage()]
    J --> K{Mode}
    K -->|Real| L[POST https://api.anthropic.com/v1/messages]
    K -->|Mock| M[Generate Random 1500-1600]
    
    L --> N[Parse API Response]
    M --> O[Return Mock Data]
    N --> P[Extract session_usage]
    O --> P
    P --> Q[Update TokenSession.tokens_used]
    Q --> R[Calculate Metrics]
    R --> S[Save to sessions.json]
    
    style C fill:#e3f2fd
    style D fill:#e8f5e8
    style G fill:#fff3e0
    style L fill:#fce4ec
    style M fill:#fff8e1
```

## Calculation Details

### Usage Rate Calculation
```
usage_rate = (current_tokens - start_tokens) / time_elapsed_minutes
```

### Efficiency Score Calculation
```
expected_rate = total_session_duration / session_progress
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

### Session Storage (sessions.json)
```json
[
  {
    "id": "uuid-v4",
    "plan_type": "Pro",
    "tokens_used": 15420,
    "tokens_limit": 40000,
    "start_time": "2025-07-08T18:30:00Z",
    "reset_time": "2025-07-08T23:30:00Z",
    "is_active": true,
    "created_at": "2025-07-08T18:30:00Z",
    "updated_at": "2025-07-08T19:15:00Z"
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

### Claude CLI Credentials (~/.claude/.credentials.json)
```json
{
  "access_token": "oauth-access-token",
  "refresh_token": "oauth-refresh-token",
  "expires_at": "2025-07-15T18:30:00Z",
  "token_type": "Bearer"
}
```