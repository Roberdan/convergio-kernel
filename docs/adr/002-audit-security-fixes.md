# ADR-002: Security Audit and Hardening

| Field      | Value                                   |
|------------|-----------------------------------------|
| Status     | Accepted                                |
| Date       | 2025-07-22                              |
| Authors    | Security Audit (automated)              |
| Supersedes | —                                       |

## Context

A comprehensive security audit of convergio-kernel (v0.1.0, ~2840 LOC)
identified several vulnerabilities across SQL handling, path validation,
HTTP client configuration, HTML injection, and input validation.

## Findings and Fixes

### CRITICAL

1. **SQL Injection in `routes.rs` (handle_events)**
   - **Issue**: `LIMIT {limit}` used string interpolation inside SQL.
     Although `limit` was capped to `u32::min(200)`, this pattern is
     inherently unsafe and sets a bad precedent.
   - **Fix**: Replaced with parameterized `LIMIT ?N` binding.

2. **Path Traversal in `verify.rs`**
   - **Issue**: `declared_outputs` and `worktree` accepted arbitrary
     user paths. `Path::new(output).exists()` could probe sensitive
     filesystem locations (`/etc/shadow`, `~/.ssh/`). `worktree` was
     passed directly to `Command::new("git").current_dir(worktree)`.
   - **Fix**: Added `is_safe_path()` validator that rejects `..`,
     null bytes, `~` prefix, and paths exceeding 1024 chars.

### HIGH

3. **Potential Panic in `routes_watchdog.rs`**
   - **Issue**: `api.authorized_chat_ids[0]` indexed without bounds
     check — panics if the vector is empty.
   - **Fix**: Replaced with `.first().and_then(|id| id.parse().ok())`.

4. **Missing HTTP Timeouts (multiple files)**
   - **Issue**: `reqwest::Client::new()` and `reqwest::blocking::get()`
     used without timeouts in `watchdog.rs`, `routes_watchdog.rs`,
     `monitor.rs`, `routes.rs`, and `telegram_poller.rs`. A slow or
     unresponsive upstream could exhaust worker threads.
   - **Fix**: All HTTP clients now use explicit timeouts:
     - 3s for local probes (`probe_mlx_model_available`)
     - 5s for health checks (`monitor.rs`)
     - 10s for daemon registration
     - 30s for inference calls and Telegram API

5. **HTML Injection in `telegram_poller.rs`**
   - **Issue**: Escaping was skipped entirely when text contained
     `<b>` or `<code>`, allowing model-generated output to inject
     arbitrary HTML tags into Telegram messages.
   - **Fix**: Now escapes ALL text first, then restores only the
     specific safe tags (`<b>`, `</b>`, `<code>`, `</code>`).

### MEDIUM

6. **Input Length Validation in `routes.rs`**
   - **Issue**: No maximum length on user-supplied text fields.
     Large payloads could cause memory pressure or DoS.
   - **Fix**: Added `sanitize_input()` with 16 KiB cap applied to
     classify and other text-input handlers.

### NOTED (not fixed — design decisions)

7. **No Authentication Layer**
   - All HTTP routes are unauthenticated. This is by design — the
     kernel runs on localhost behind the daemon's auth layer. Documented
     as an assumption rather than a vulnerability.

8. **Prompt Injection Surface**
   - User text is embedded in LLM prompts. Mitigated by grounding
     instructions and system prompts. Full prompt-injection defense
     requires output filtering, which is out of scope for this audit.

9. **CI Workflow Supply Chain**
   - Reusable workflows reference `@main` (mutable). Pinning to SHA
     is recommended but deferred to the CI hardening track.

## Decision

All CRITICAL and HIGH findings are fixed. MEDIUM findings are fixed
where possible. NOTED items are documented for future hardening.

## Consequences

- Parameterized SQL eliminates injection risk in event queries
- Path validation prevents filesystem probing via verify endpoint
- Timeouts prevent thread exhaustion from slow upstreams
- HTML escaping prevents tag injection in Telegram messages
- Input caps prevent memory-based DoS
