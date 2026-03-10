# pj-receive

A Taproot-optimized Payjoin v2 (BIP 77) receiver with a privacy-aware coin selection engine and interactive terminal UI.

`pj-receive` acts as a standalone BIP 77 receiver that connects to a local Bitcoin Core node, manages receiver-side UTXO selection with Taproot optimization, and communicates asynchronously through the Payjoin Directory infrastructure. Its core contribution is a multi-heuristic scoring algorithm that balances privacy, fee efficiency, and UTXO consolidation when choosing which receiver inputs to contribute to a Payjoin transaction.

The application offers two interfaces: a headless **CLI** for scripting and automation, and an interactive **TUI (Terminal UI)** for hands-on use with real-time wallet monitoring, guided session creation, and live coin selection previews.

## Table of Contents

- [Background](#background)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
  - [Terminal UI (TUI)](#terminal-ui-tui)
  - [Command-Line Interface (CLI)](#command-line-interface-cli)
- [Coin Selection Engine](#coin-selection-engine)
- [Project Structure](#project-structure)
- [Development](#development)
- [Testing](#testing)
- [Contributing](#contributing)
- [License](#license)

## Background

Payjoin (BIP 77/78) is the most effective single-transaction privacy technique in Bitcoin. By having both the sender and receiver contribute inputs to a payment transaction, Payjoin breaks the **common-input-ownership heuristic** — the single most relied-upon assumption in blockchain surveillance.

The [Payjoin Dev Kit (PDK)](https://github.com/payjoin/rust-payjoin) provides the protocol machinery but **explicitly delegates** coin selection to the integrator. Poor coin selection can reduce privacy, increase fees, or leak information through script-type mismatches. This project fills that gap with an intelligent, Taproot-native selection engine.

### Relevant BIPs

| BIP | Title | Role in this project |
|-----|-------|---------------------|
| [BIP 77](https://github.com/bitcoin/bips/blob/master/bip-0077.mediawiki) | Payjoin Version 2: Serverless Payjoin | Core protocol (asynchronous, directory-based) |
| [BIP 78](https://github.com/bitcoin/bips/blob/master/bip-0078.mediawiki) | Payjoin (V1) | Backward-compatible synchronous mode |
| [BIP 21](https://github.com/bitcoin/bips/blob/master/bip-0021.mediawiki) | URI Scheme | Payment URI generation with `pj=` parameter |
| [BIP 341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki) | Taproot | Preferred input/output type (P2TR) |

## Architecture

```
                         ┌──────────────────────────────────────────┐
                         │              pj-receive                  │
                         │                                          │
                         │  ┌──────────────────────────────────┐    │
                         │  │     User Interface               │    │
                         │  │  CLI (clap) / TUI (ratatui)      │    │
                         │  └───────────────┬──────────────────┘    │
                         │                  │                       │
                         │  ┌───────────────▼──────────────────┐    │
                         │  │       Session Manager            │    │
                         │  │  BIP 21 URI / lifecycle / state  │    │
                         │  └───────┬──────────────┬───────────┘    │
                         │          │              │                │
                         │  ┌───────▼──────┐  ┌────▼───────────┐   │
                         │  │  Protocol    │  │ Coin Selection │   │
                         │  │  Engine      │  │ Engine         │   │
                         │  │  (PDK v2)    │  │                │   │
                         │  │  OHTTP/HPKE  │  │ UTXO Scoring   │   │
                         │  │  PSBT        │  │ Script-Type    │   │
                         │  │  Validation  │  │ Decorrelation  │   │
                         │  └───────┬──────┘  └────┬───────────┘   │
                         │          │              │                │
                         │  ┌───────▼──────────────▼───────────┐   │
                         │  │    Bitcoin Core RPC Interface     │   │
                         │  └───────────────┬──────────────────┘   │
                         └──────────────────┼──────────────────────┘
                                            │
                  ┌─────────────────────────┼─────────────────────────┐
                  │                         │                         │
         ┌────────▼──────┐      ┌───────────▼────────┐   ┌───────────▼────────┐
         │ Bitcoin Core  │      │   OHTTP Relay      │   │ Payjoin Directory  │
         │ (signet/      │      │   (IP privacy)     │   │ (async mailbox)    │
         │  regtest)     │      └────────────────────┘   └────────────────────┘
         └───────────────┘
```

**Data flow for receiving a Payjoin payment:**

1. **Init** — Fetch wallet UTXOs, register session with Payjoin Directory via OHTTP relay, display BIP 21 URI
2. **Poll** — Asynchronously poll the directory for the sender's Original PSBT
3. **Validate** — Check inputs not owned, no replays, identify receiver outputs
4. **Select** — Score and select receiver UTXOs via the coin selection engine
5. **Propose** — Insert receiver inputs, sign via Bitcoin Core, submit Proposal PSBT
6. **Confirm** — Sender retrieves proposal, validates, re-signs, and broadcasts

## Prerequisites

- **Rust** >= 1.85 (install via [rustup](https://rustup.rs/))
- **Bitcoin Core** >= 27.0 with wallet support enabled
  - Must be running on signet or regtest for development
  - Requires JSON-RPC access with authentication
- **payjoin-cli** (optional, for end-to-end testing as sender)

### Installing Bitcoin Core

Download from [bitcoincore.org](https://bitcoincore.org/en/download/) or build from source.

Example `bitcoin.conf` for **signet**:

```ini
signet=1
server=1
rpcuser=your_rpc_user
rpcpassword=your_rpc_password
rpcport=38332

# Wallet support
disablewallet=0
```

Example `bitcoin.conf` for **regtest** (recommended for development):

```ini
regtest=1
server=1
rpcuser=your_rpc_user
rpcpassword=your_rpc_password
rpcport=18443

disablewallet=0
fallbackfee=0.00001
```

Start Bitcoin Core:

```bash
bitcoind -daemon
```

Create and fund a wallet:

```bash
# Create a descriptor wallet
bitcoin-cli -signet createwallet "pj-wallet" true true "" false true

# For regtest, generate blocks to fund the wallet
bitcoin-cli -regtest -rpcwallet=pj-wallet -generate 101
```

### Installing payjoin-cli (for testing)

```bash
cargo install payjoin-cli
```

## Installation

Clone the repository and build:

```bash
git clone https://github.com/AfriBTC/Taproot-Optimized-Payjoin-Receiver.git
cd Taproot-Optimized-Payjoin-Receiver
cargo build --release
```

The binary will be at `target/release/pj-receive`.

Or run directly with Cargo:

```bash
cargo run -- <command> [options]
```

## Configuration

Initialize `pj-receive` with your Bitcoin Core connection details:

```bash
pj-receive init \
  --rpc-host 127.0.0.1 \
  --rpc-port 38332 \
  --rpc-user your_rpc_user \
  --rpc-pass your_rpc_password \
  --wallet pj-wallet \
  --network signet
```

This creates a configuration file at `~/.pj-receive/config.json`. You can view it with:

```bash
pj-receive config --show
```

All global options (`--rpc-host`, `--rpc-port`, etc.) can be passed to any command to override the saved configuration.

## Usage

### Terminal UI (TUI)

The recommended way to use `pj-receive` interactively. Launch with:

```bash
pj-receive tui
```

The TUI provides a full-screen terminal interface with four screens:

```
┌─ pj-receive ──────────────────────────────────────────────────────────┐
│  Connected │ Network: signet │ Block: 8501 │ UTXOs: 3 │ Balance: ... │
├───────────────────────────────────────────────────────────────────────┤
│ [1] Dashboard │ [2] New Session │ [3] Sessions │ [?] Help            │
├─ Wallet Summary ──────────────────────────────────────────────────────┤
│  Taproot (P2TR): 3    SegWit (P2WPKH): 0    Fee Rate: 1.0 sat/vB   │
├─ Wallet UTXOs ────────────────────────────────────────────────────────┤
│  #  Type   Amount (sats)   Confs   Address                           │
│  1  P2TR   1,000,000       1       tb1pqtw2...ftxk4fs               │
│  2  P2TR     500,000       1       tb1pw87c...a97347                 │
│  3  P2TR     250,000       1       tb1peqq3...cenksy                 │
├─ Log ─────────────────────────────────────────────────────────────────┤
│  12:28:44 INFO Connected to Bitcoin Core (signet), block 8501        │
│  12:28:44 INFO Loaded 3 UTXOs                                        │
└───────────────────────────────────────────────────────────────────────┘
 q Quit  1-3 Navigate  ? Help  r Refresh
```

#### TUI Screens

**[1] Dashboard** — Main overview showing connection status, wallet UTXOs with script types, balance, fee rate, and a live activity log. Data auto-refreshes every 10 seconds.

**[2] New Session** — Interactive form to create a Payjoin receive session. Fill in the amount, choose a strategy, and press Enter to start. The coin selection preview updates live as you type the amount, showing which UTXOs will be selected and their scores.

**[3] Sessions** — Lists all active and completed Payjoin sessions with their status (Pending, Proposal Sent, Completed, Expired, Failed).

**[?] Help** — Keyboard shortcuts reference and coin selection strategy explanations.

#### TUI Keyboard Shortcuts

| Key | Context | Action |
|-----|---------|--------|
| `1` `2` `3` | Any screen | Switch between Dashboard, New Session, Sessions |
| `?` | Any screen | Toggle help screen |
| `r` | Any screen | Refresh wallet data from Bitcoin Core |
| `q` | Normal mode | Quit the application |
| `Ctrl+C` | Any | Force quit |
| `Tab` / `Down` | New Session form | Move to next field |
| `Shift+Tab` / `Up` | New Session form | Move to previous field |
| `Enter` | New Session form | Start the receive session |
| `Esc` | New Session / Help | Go back to Dashboard |

#### TUI New Session Form Fields

| Field | Default | Description |
|-------|---------|-------------|
| Amount (sats) | required | Payment amount the sender will pay |
| Strategy | `balanced` | Coin selection profile (see [Strategy Profiles](#strategy-profiles)) |
| Max Inputs | `1` | Maximum number of receiver UTXOs to contribute |
| Expiry (min) | `60` | How long the session stays active |
| Label | optional | Human-readable note for your records |

### Command-Line Interface (CLI)

The headless CLI is available for scripting, automation, and non-interactive use. All TUI functionality is also accessible via CLI commands.

#### Receive a Payjoin Payment

```bash
pj-receive receive \
  --amount 100000 \
  --strategy balanced \
  --label "Payment from Alice"
```

This will:
1. Preview the coin selection (showing which UTXOs will be contributed)
2. Generate a BIP 21 URI with the `pj=` parameter
3. Poll the Payjoin Directory for the sender's PSBT
4. Validate, construct, sign, and submit the Payjoin proposal

**Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `--amount <SATS>` | required | Expected payment amount in satoshis |
| `--strategy <PROFILE>` | `balanced` | Coin selection strategy (see below) |
| `--max-inputs <N>` | `1` | Maximum receiver inputs to contribute |
| `--expiry <MINUTES>` | `60` | Session expiry timeout |
| `--label <STRING>` | none | Human-readable label for the BIP 21 URI |
| `--directory <URL>` | `https://payjo.in` | Payjoin Directory URL |
| `--ohttp-relay <URL>` | `https://pj.bobspacebkk.com` | OHTTP relay URL |

#### Other CLI Commands

```bash
# Check active sessions
pj-receive status

# View completed transactions
pj-receive history

# View current configuration
pj-receive config --show

# Launch the interactive TUI
pj-receive tui
```

#### Verbosity

Add `-v`, `-vv`, or `-vvv` to any CLI command for increasing log detail:

```bash
pj-receive -vv receive --amount 100000
```

## Coin Selection Engine

The coin selection engine is the core novel contribution of this project. It scores candidate UTXOs across five dimensions and selects the optimal set for privacy and fee efficiency.

### Scoring Dimensions

| Dimension | Max Weight | Description |
|-----------|-----------|-------------|
| **Script-type** | 30 | P2TR (1.0) > P2WPKH (0.67) > P2SH (0.33) > P2PKH (0.1). Taproot key-path spends are 57.5 vB vs 68+ for P2WPKH and all Taproot key-path outputs are indistinguishable on-chain. |
| **Amount decorrelation** | 25 | Measures how well a UTXO obscures the payment amount. Penalizes UTXOs whose value closely matches the payment, round-number amounts, and unbalanced output splits. |
| **Fee efficiency** | 20 | Ratio of spending cost to UTXO value. High-value UTXOs with low-weight script types score highest. Dust UTXOs that cost more to spend than they're worth score near zero. |
| **Consolidation** | 15 | Small UTXOs (< 50,000 sats) during low-fee periods (< 5 sat/vB) score highest, turning privacy transactions into free consolidation events. |
| **UTXO age** | 10 | Gaussian distribution centered at ~144 blocks (~1 day). Very fresh UTXOs may link to identifiable recent transactions; very old UTXOs may be recognizable as long-dormant holdings. |

### Strategy Profiles

Each profile adjusts the dimension weights:

| Profile | Script | Decorrelation | Fee | Consolidation | Age | Use case |
|---------|--------|---------------|-----|---------------|-----|----------|
| `balanced` | 30 | 25 | 20 | 15 | 10 | Default. Good all-around privacy and efficiency. |
| `privacy-max` | 35 | 40 | 5 | 5 | 15 | Maximum heuristic breakage. Ignores fee cost. |
| `fee-min` | 25 | 10 | 45 | 10 | 10 | Minimize fee impact. Choose cheapest inputs. |
| `consolidate` | 10 | 10 | 15 | 55 | 10 | Clean up dust UTXOs during low-fee periods. |

### Scoring Algorithm

```
for each spendable UTXO in wallet:
    score = (script_type_score   * weight.script_type)
          + (decorrelation_score * weight.decorrelation)
          + (fee_efficiency      * weight.fee_efficiency)
          + (consolidation_score * weight.consolidation)
          + (age_score           * weight.age)

select top-scoring UTXOs up to max_inputs
```

The engine also attempts **UIH (Unnecessary Input Heuristic) avoidance** via `try_preserving_privacy()` before falling back to contributing all selected inputs.

## Project Structure

```
pj-receive/
├── Cargo.toml
├── src/
│   ├── main.rs                     # Entry point, dispatches to CLI or TUI
│   ├── cli.rs                      # Clap command/argument definitions
│   ├── config.rs                   # Configuration loading, saving, overrides
│   ├── rpc.rs                      # Bitcoin Core RPC wrapper
│   ├── protocol.rs                 # BIP 77 v2 receiver flow (13-step pipeline)
│   ├── session.rs                  # Session state, replay protection
│   ├── persistence.rs              # JSON-based session storage
│   ├── logging.rs                  # Structured logging (tracing)
│   ├── coin_selection/             # Core coin selection engine
│   │   ├── mod.rs                  # Public API: select_inputs()
│   │   ├── scorer.rs               # Multi-heuristic UTXO scoring
│   │   ├── script_types.rs         # Script-type detection and weight calculation
│   │   ├── strategies.rs           # Strategy profiles and weight configurations
│   │   └── decorrelation.rs        # Amount decorrelation analysis
│   └── tui/                        # Interactive terminal UI
│       ├── mod.rs                  # Terminal setup, main event loop
│       ├── app.rs                  # Application state, wallet refresh, session logic
│       ├── ui.rs                   # Screen layouts and widget rendering
│       └── event.rs                # Keyboard input and tick event polling
└── tests/
    ├── unit/                       # Additional unit tests
    └── integration/                # End-to-end tests on regtest
```

### Key Modules

| Module | Responsibility |
|--------|----------------|
| `protocol.rs` | Orchestrates the full BIP 77 receiver flow — session init, OHTTP communication, PSBT validation, proposal construction, signing, and response. |
| `coin_selection/` | Self-contained scoring engine. Designed to be reusable and suitable for upstream contribution to PDK. The public API is a single function: `select_inputs()`. |
| `rpc.rs` | Thin wrapper around `bitcoincore-rpc`. Handles connection, address generation (Bech32m), UTXO listing, PSBT signing, fee estimation, and script ownership checks. |
| `tui/` | Interactive terminal UI built with `ratatui` and `crossterm`. Provides a 4-screen interface (Dashboard, New Session, Sessions, Help) with live wallet monitoring and guided session creation. |
| `session.rs` | Manages session lifecycle and seen-inputs tracking for replay protection. |
| `persistence.rs` | Serializes session state to JSON files so sessions survive process restarts. |

## Development

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Check compilation without building
cargo check
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Check for security advisories in dependencies
cargo audit
```

### Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `payjoin` | 0.21.0 | BIP 77/78 protocol (v2 + receive + io features) |
| `bitcoin` | 0.32.8 | Transaction types, script types, amounts |
| `bitcoincore-rpc` | 0.19.0 | Bitcoin Core JSON-RPC client |
| `tokio` | 1.x | Async runtime for polling and network I/O |
| `reqwest` | 0.12 | HTTP client for OHTTP relay communication |
| `clap` | 4.x | CLI argument parsing with derive macros |
| `ratatui` | 0.29 | Terminal UI framework (widgets, layout, rendering) |
| `crossterm` | 0.28 | Cross-platform terminal manipulation (raw mode, events) |
| `serde` / `serde_json` | 1.x | Configuration and session serialization |
| `tracing` | 0.1 | Structured logging |
| `anyhow` | 1.x | Error handling |
| `url` | 2.x | URL parsing and validation |

## Testing

### Run All Tests

```bash
cargo test
```

There are currently 31 unit tests covering the coin selection engine, session management, persistence, and configuration.

### Test Categories

**Coin selection (`coin_selection/`)**
- Script-type detection against known scriptPubKey hex patterns
- Scoring algorithm edge cases (dust, high-value, round numbers)
- Strategy weight validation (all profiles sum to 100)
- Input selection ordering and filtering
- Decorrelation analysis (similar amounts, zero values, balance)

**Session (`session.rs`)**
- Seen-inputs tracking for replay protection
- Session status display formatting

**Persistence (`persistence.rs`)**
- Save/load round-trip
- Upsert (insert + update) behavior
- Empty state handling

**RPC (`rpc.rs`)**
- Network string parsing

### End-to-End Testing on Regtest

To run a full Payjoin flow locally:

1. **Start Bitcoin Core on regtest:**
   ```bash
   bitcoind -regtest -daemon \
     -rpcuser=test -rpcpassword=test -rpcport=18443 \
     -fallbackfee=0.00001
   ```

2. **Create and fund two wallets** (receiver and sender):
   ```bash
   # Receiver wallet
   bitcoin-cli -regtest createwallet "receiver" true true "" false true
   bitcoin-cli -regtest -rpcwallet=receiver -generate 101

   # Sender wallet
   bitcoin-cli -regtest createwallet "sender" true true "" false true
   SENDER_ADDR=$(bitcoin-cli -regtest -rpcwallet=sender getnewaddress "" bech32m)
   bitcoin-cli -regtest -rpcwallet=receiver sendtoaddress "$SENDER_ADDR" 1.0
   bitcoin-cli -regtest -rpcwallet=receiver -generate 1
   ```

3. **Initialize pj-receive:**
   ```bash
   cargo run -- init \
     --rpc-host 127.0.0.1 --rpc-port 18443 \
     --rpc-user test --rpc-pass test \
     --wallet receiver --network regtest
   ```

4. **Start the receiver (TUI or CLI):**
   ```bash
   # Interactive TUI
   cargo run -- tui

   # Or headless CLI
   cargo run -- -vv receive --amount 50000 --strategy balanced
   ```

5. **Send from payjoin-cli** (in another terminal):
   ```bash
   payjoin-cli send --bip21 "<URI from step 4>" \
     --rpc-host 127.0.0.1 --rpc-port 18443 \
     --rpc-user test --rpc-pass test \
     --wallet sender
   ```

### Writing New Tests

Unit tests live alongside each module using `#[cfg(test)]` blocks. See `src/coin_selection/scorer.rs` for well-documented test examples.

For integration tests, add files to `tests/integration/`. These should spin up Bitcoin Core on regtest programmatically or use pre-configured test fixtures.

## Contributing

Contributions are welcome. This project is part of the [BOSS (Bitcoin Open-Source Software) Challenge 2026](https://learning.chaincode.com/) program.

### Getting Started

1. **Fork** the repository
2. **Clone** your fork:
   ```bash
   git clone https://github.com/<your-username>/Taproot-Optimized-Payjoin-Receiver.git
   cd Taproot-Optimized-Payjoin-Receiver
   ```
3. **Create a branch** for your work:
   ```bash
   git checkout -b feature/your-feature-name
   ```
4. **Build and test** to verify your setup:
   ```bash
   cargo build && cargo test
   ```
5. **Launch the TUI** to explore the application:
   ```bash
   cargo run -- tui
   ```

### Areas for Contribution

| Area | Description | Difficulty |
|------|-------------|------------|
| **Integration tests** | End-to-end tests on regtest with crafted UTXO sets | Medium |
| **BIP 78 v1 support** | Backward-compatible synchronous mode for v1 senders | Medium |
| **Strategy tuning** | Benchmark strategies against UTXO datasets, improve weight calibration | Medium |
| **Privacy analysis** | Validate coin selection against known chain analysis heuristics | Hard |
| **Output substitution** | Implement receiver output substitution for enhanced privacy | Medium |
| **Transaction monitoring** | Watch for confirmation after proposal is sent | Easy |
| **TUI: QR code display** | Render BIP 21 URIs as QR codes in the New Session screen | Easy |
| **TUI: Session detail view** | Expand a session row to show full URI, PSBT details, and timeline | Medium |
| **TUI: UTXO selection highlighting** | Highlight which UTXOs are selected for the current session | Easy |
| **TUI: Custom themes** | Support user-configurable color themes | Easy |
| **Error recovery** | Graceful handling of partial failures, session resume after crash | Medium |

### Code Style and Standards

- **Format** all code with `cargo fmt` before committing
- **Lint** with `cargo clippy -- -D warnings` — no warnings allowed
- **Test** all new functionality — run `cargo test` and ensure all tests pass
- Follow the existing module structure and naming conventions
- Keep the coin selection engine (`src/coin_selection/`) decoupled from protocol logic — it should remain independently testable and suitable for upstream contribution
- Keep TUI rendering (`src/tui/ui.rs`) separate from application state (`src/tui/app.rs`) — the UI should be a pure function of app state
- Use `anyhow::Result` for application-level errors and typed errors at module boundaries
- Use `tracing` macros (`tracing::info!`, `tracing::debug!`) for logging, not `println!` (except for user-facing CLI output)

### Commit Messages

Write clear, concise commit messages:

```
Add UTXO age scoring to coin selection engine

Implement a Gaussian-based age scoring function centered at 144 blocks
(~1 day) that penalizes very fresh and very old UTXOs. This reduces the
risk of linking receiver inputs to identifiable recent or dormant
transactions.
```

- Use imperative mood ("Add", "Fix", "Update", not "Added", "Fixed")
- First line under 72 characters
- Body explains **why**, not just **what**

### Pull Request Process

1. Ensure `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` all pass
2. Write a clear PR description with:
   - Summary of changes
   - Motivation / problem being solved
   - Testing done
3. Link any related issues
4. PRs are reviewed for correctness, test coverage, and adherence to the project's privacy-first design goals

### Reporting Issues

Open an issue on GitHub with:
- A clear description of the bug or feature request
- Steps to reproduce (for bugs)
- Expected vs actual behavior
- Your environment (OS, Rust version, Bitcoin Core version)

## License

This project is licensed under the [MIT License](LICENSE).

## Acknowledgments

- [Payjoin Dev Kit](https://github.com/payjoin/rust-payjoin) — protocol implementation
- [Btrust Builders](https://www.btrust.tech/) and [Chaincode Labs](https://chaincode.com/) — BOSS Challenge program
- [Dan Gould](https://github.com/DanGould) — PDK maintainer and Payjoin protocol design
- [ratatui](https://github.com/ratatui/ratatui) — terminal UI framework
