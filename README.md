# ccost - Claude Cost Tracking Tool

<div align="center">

**Accurate Claude API usage tracking with intelligent deduplication**

[![GitHub Release](https://img.shields.io/github/v/release/carlosarraes/ccost?style=flat&color=blue)](https://github.com/carlosarraes/ccost/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)

</div>

## 🎯 What is ccost?

ccost is a comprehensive Claude API usage tracking and cost analysis tool designed to provide accurate insights into your AI usage patterns. It features intelligent message deduplication using requestId priority to ensure accurate cost calculations aligned with API billing.

### ✅ Key Features:
- ✅ **Live pricing with LiteLLM integration** - Real-time model pricing with granular cache costs (creation vs read rates)
- ✅ **Dual caching system** - 24-hour persistent caching for both currency rates and model pricing
- ✅ **Enhanced deduplication** using requestId priority with sessionId fallback for optimal billing accuracy
- ✅ **Intuitive CLI** with direct commands (no nested subcommands)
- ✅ **Multi-currency support** with live exchange rates (EUR, GBP, JPY, CNY, BRL, etc.)
- ✅ **Project filtering** with comma-separated support for multiple projects
- ✅ **SQLite caching** for offline operation and improved performance
- ✅ **Timezone-aware** daily cutoffs and filtering
- ✅ **Comprehensive filtering** by date ranges, models, and projects
- ✅ **Privacy mode** with --hidden flag for sensitive project names
- ✅ **Zero dependencies** - Self-contained binary, no Node.js required

## 🚀 Inspiration

ccost was created as a self-contained alternative to [ccusage](https://github.com/ryoppippi/ccusage) - an excellent Claude usage analysis tool. While ccusage provides great functionality, ccost addresses specific needs:

- **🔧 Zero Dependencies**: ccost is a single binary with no Node.js dependency, eliminating version conflicts in development environments where Node versions frequently change
- **💱 Multi-Currency Support**: Built-in currency conversion with 24-hour caching for international users  
- **🎯 Enhanced Accuracy**: When this project started, ccusage had deduplication issues (since resolved). ccost was designed with billing-aligned deduplication from the ground up
- **⚡ Performance**: Dual caching system (currency + pricing) with persistent file-based storage for consistent sub-second response times

Both tools now provide excellent Claude usage analysis - choose based on your preferences for runtime dependencies and specific feature requirements.

## 📢 What's New in v0.2.0

**🚨 BREAKING CHANGES**: ccost v0.2.0 introduces major enhancements:

### 🆕 **New Features**
- ✅ **Live LiteLLM Pricing**: Real-time model pricing with granular cache cost differentiation
- ✅ **Persistent Caching**: 24-hour file-based caching for both currency rates and model pricing
- ✅ **Configuration System**: Comprehensive TOML configuration with pricing source options (static/live/auto)
- ✅ **Enhanced Accuracy**: <1% cost variance compared to live pricing tools

### 🔄 **CLI Improvements** 
- ✅ **Simplified Commands**: `ccost today` instead of `ccost usage today`
- ✅ **Enhanced Projects**: `ccost projects proj1,proj2` for multiple project filtering
- ✅ **Better Deduplication**: requestId priority for improved billing accuracy
- ✅ **Privacy Mode**: `--hidden` flag for sensitive project names
- ✅ **Default Overview**: `ccost` (no args) shows complete usage summary

**Migration Guide**: Replace `ccost usage <timeframe>` with `ccost <timeframe>` and update project commands to use comma-separated filtering.

## 🚀 Quick Start

### One-Line Installation

```bash
curl -sSf https://raw.githubusercontent.com/carlosarraes/ccost/main/install.sh | sh
```

### Basic Usage

```bash
# View overall usage summary
ccost

# View today's usage
ccost today

# View this week's usage in EUR
ccost this-week --currency EUR

# Analyze specific projects
ccost projects project1,project2

# View detailed daily breakdown
ccost daily --days 7
```

## 📊 Features

### 🔍 Usage Analysis
- **Direct timeframe commands**: `today`, `yesterday`, `this-week`, `this-month`, `daily`
- **Global filtering**: `--model claude-sonnet-4`, `--since 2025-01-01`, `--until 2025-01-31`
- **Enhanced deduplication**: requestId priority with sessionId fallback for billing accuracy
- **Privacy mode**: `--hidden` flag to obscure sensitive project names
- **Verbose statistics**: See exactly how many duplicate messages were filtered

### 💰 Multi-Currency Support
- **Real-time conversion** via European Central Bank API
- **Persistent 24-hour caching** for offline usage and performance
- **Supported currencies**: USD, EUR, GBP, JPY, CNY, BRL, and more
- **Proper formatting**: $12.34, €10.45, £8.99, ¥1,234
- **Cache location**: `~/.config/ccost/currency_cache.json`

### 📈 Project Analysis
- **Comma-separated filtering**: `ccost projects project1,project2,project3`
- **Smart project detection** from file paths and `cwd` fields
- **Unified table output**: All requested projects in a single view
- **Accurate totals**: Sum only the requested projects, not all projects
- **Usage statistics**: total tokens, costs, and model distribution

### ⚙️ Configuration Management
```bash
# View current config
ccost config show

# Initialize fresh config
ccost config init

# Set configuration values
ccost config set currency.default_currency EUR
ccost config set timezone.timezone "America/New_York"
ccost config set output.date_format "dd-mm-yyyy"
ccost config set pricing.source live              # Use live LiteLLM pricing
```

### 🎯 Pricing Modes
- **Static pricing** (default): Fast, uses embedded pricing data
- **Live pricing**: Real-time LiteLLM pricing with granular cache costs  
- **Auto pricing**: Live pricing with static fallback when offline
- **Persistent caching**: 24-hour file-based cache at `~/.config/ccost/litellm_cache.json`

## 📋 Command Reference

### Overview & Basic Commands
```bash
# Overall usage summary (default behavior)
ccost                                 # Show all projects with totals

# Direct timeframe commands (no nested subcommands)
ccost today                           # Today's usage
ccost yesterday                       # Yesterday's usage  
ccost this-week                       # This week's usage
ccost this-month                      # This month's usage
ccost daily                           # Daily breakdown (7 days)
ccost daily --days 30                 # Daily breakdown (30 days)
```

### Global Options (Available on All Commands)
```bash
# Filtering options
--model claude-sonnet-4               # Filter by model
--since 2025-01-01                    # Start date
--until 2025-01-31                    # End date
--currency EUR                        # Convert to specific currency
--timezone "America/New_York"         # Override timezone

# Output options
--json                                # JSON output format
--verbose                             # Detailed statistics
--colored                             # Enable colored output
--hidden                              # Privacy mode (dummy project names)
```

### Project Analysis
```bash
# Project filtering and analysis
ccost projects                        # Show all projects
ccost projects myproject              # Show specific project
ccost projects proj1,proj2,proj3      # Show multiple projects (comma-separated)
ccost projects --hidden               # Show projects with privacy mode
```

### Configuration Management
```bash
ccost config show                     # Display current configuration
ccost config init                     # Create fresh config file
ccost config set key value            # Set configuration value
```

## 🔧 Configuration

ccost stores configuration at `~/.config/ccost/config.toml`:

```toml
[general]
claude_projects_path = "~/.claude/projects"
cost_mode = "auto"

[currency]
default_currency = "USD"

[timezone]
timezone = "UTC"
daily_cutoff_hour = 0

[output]
colored = true
decimal_places = 2
date_format = "yyyy-mm-dd"  # Options: "yyyy-mm-dd", "dd-mm-yyyy", "mm-dd-yyyy"

[pricing]
source = "auto"              # Options: "static", "live", "auto"
cache_ttl_minutes = 60       # In-memory cache TTL
offline_fallback = true      # Fallback to static when live pricing fails
```

### Supported Currencies
- **USD** (US Dollar) - Default
- **EUR** (Euro)
- **GBP** (British Pound)
- **JPY** (Japanese Yen)
- **CNY** (Chinese Yuan)
- **BRL** (Brazilian Real)
- And more via ECB API

### Timezone Support
ccost supports all standard timezone identifiers:
- `UTC`
- `America/New_York`
- `Europe/London`
- `Asia/Tokyo`
- `Australia/Sydney`
- And 400+ more via chrono-tz

## 🎨 Output Examples

### Today's Usage
```bash
$ ccost today --hidden
```
```
 Project           Input Tokens   Output Tokens   Cache Creation   Cache Read   Messages   Total Cost 
──────────────────────────────────────────────────────────────────────────────────────────────────────
 project-28                 245           1,244          482,261    4,481,930         63        $1.68 
 project-36                 659           5,641          728,386    9,223,010        135        $4.34 
 project-37                  53             402            9,277      146,087          9        $0.05 
 project-rho                189           2,186          126,856      883,775         26        $0.41 
 project-upsilon          2,304         165,573        1,053,202   23,954,993        349       $12.53 
 project-34               1,949         122,381          724,054   28,828,721        413       $10.86 
──────────────────────────────────────────────────────────────────────────────────────────────────────
 TOTAL                    5,399         297,427        3,124,036   67,518,516        995       $29.87
```

### Specific Projects Analysis
```bash
$ ccost projects project-upsilon,project-rho --hidden
```
```
 Project         Input Tokens   Output Tokens   Cache Creation   Cache Read    Messages   Total Cost 
─────────────────────────────────────────────────────────────────────────────────────────────────────
 project-34             6,308         608,841        5,144,311   106,512,695      1,630       $43.98 
 project-kappa         96,107       1,924,201       20,103,406   606,072,529      7,453      $434.98 
─────────────────────────────────────────────────────────────────────────────────────────────────────
 TOTAL                102,415       2,533,042       25,247,717   712,585,224      9,083      $478.97
```

### Daily Breakdown (Last 3 Days)
```bash
$ ccost daily --days 3 --hidden
```
```
 Date         Input Tokens   Output Tokens   Cache Creation   Cache Read    Messages   Projects   Total Cost 
─────────────────────────────────────────────────────────────────────────────────────────────────────────────
 2025-06-18         15,765         113,883        4,130,141    47,699,240        763          8       $27.10 
 2025-06-19         11,543         820,279        7,460,526   131,888,399      2,081          5       $57.77 
 2025-06-20          5,417         297,492        3,125,423    67,774,610        998          6       $29.95 
─────────────────────────────────────────────────────────────────────────────────────────────────────────────
 TOTAL              32,725       1,231,654       14,716,090   247,362,249      3,842         19      $114.81
```

## 🏗️ Architecture

ccost v0.2.0 is built with a robust, modular architecture:

- **Parser Module**: JSONL parsing with full Claude data structure support
- **Enhanced Deduplication Engine**: requestId priority with sessionId fallback for billing accuracy
- **Database Layer**: SQLite with WAL mode for persistence and caching
- **Dual Caching System**: 
  - **Currency Manager**: ECB API integration with 24-hour persistent caching
  - **LiteLLM Integration**: Live model pricing with 24-hour persistent caching
- **Enhanced Pricing Engine**: Live LiteLLM pricing with granular cache cost differentiation
- **Analysis Engine**: Usage tracking, project analysis, and accurate cost calculation
- **Configuration System**: Comprehensive TOML configuration with pricing source management
- **Simplified CLI Framework**: Direct command structure without nested subcommands

### Data Flow
1. **Initialize** pricing manager with live LiteLLM data (cached for 24h)
2. **Parse** JSONL files from `~/.claude/projects/`
3. **Deduplicate** messages using requestId priority strategy
4. **Filter** projects with comma-separated support
5. **Calculate** costs using enhanced pricing with granular cache rates
6. **Convert** currencies using cached exchange rates (24h TTL)
7. **Cache** results in SQLite for performance
8. **Display** results with professional formatting and privacy mode

## 🔍 Enhanced Deduplication Strategy (v0.2.0)

ccost now uses a billing-aligned deduplication strategy optimized for API accuracy:

1. **Priority 1**: `message.id + requestId` (optimal for API billing alignment)
2. **Priority 2**: `message.id + sessionId` (fallback when requestId unavailable)
3. **No Hash Generation**: Messages without both message.id and identifier are excluded

This strategy provides:
- **Better billing accuracy** aligned with Claude API billing practices
- **Improved deduplication rates** (target ~18% vs previous ~12%)
- **Simplified logic** without complex multi-tier fallbacks
- **Hash collision prevention** with "req:" and "session:" prefixes

### Deduplication Statistics
ccost provides detailed deduplication reporting with `--verbose`:
- **Total messages found**: Raw count from JSONL files
- **Duplicates removed**: Number of duplicate messages filtered
- **Deduplication rate**: Percentage of duplicates (improved ~18% target)
- **Unique messages**: Final count used for cost calculation

## 🚀 Installation Options

### Option 1: One-Line Install (Recommended)
```bash
curl -sSf https://raw.githubusercontent.com/carlosarraes/ccost/main/install.sh | sh
```

### Option 2: Manual Download
1. Visit [Releases](https://github.com/carlosarraes/ccost/releases)
2. Download the binary for your platform:
   - `ccost-linux-x86_64.tar.gz` (Linux)
   - `ccost-macos-x86_64.tar.gz` (Intel Mac)
   - `ccost-macos-aarch64.tar.gz` (Apple Silicon Mac)
3. Extract and move to `$PATH`

### Option 3: Build from Source
```bash
git clone https://github.com/carlosarraes/ccost.git
cd ccost
cargo build --release
sudo cp target/release/ccost /usr/local/bin/
```

### Supported Platforms
- ✅ **Linux x86_64** (with musl for static linking)
- ✅ **macOS x86_64** (Intel)
- ✅ **macOS aarch64** (Apple Silicon)

## 🛠️ Development

### Prerequisites
- Rust 1.70+ with 2024 edition support
- SQLite development libraries

### Building
```bash
git clone https://github.com/carlosarraes/ccost.git
cd ccost
cargo build --release
```

### Testing
```bash
cargo test                    # Run unit tests
cargo test --test integration # Run integration tests
```

### Key Dependencies
- **clap**: CLI framework and argument parsing
- **serde**: JSON/TOML serialization
- **chrono**: Date/time handling with timezone support
- **tokio**: Async runtime for HTTP requests
- **reqwest**: HTTP client for API calls
- **tabled**: Professional table formatting

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development Guidelines
- Write tests for new features
- Follow Rust conventions and run `cargo fmt`
- Update documentation for user-facing changes
- Ensure CI passes before submitting PR

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/carlosarraes/ccost/issues)
- **Discussions**: [GitHub Discussions](https://github.com/carlosarraes/ccost/discussions)
- **Documentation**: This README and inline help (`ccost --help`)

---

<div align="center">

**Made with ❤️ for the Claude community**

[⭐ Star this repo](https://github.com/carlosarraes/ccost) • [🐛 Report Bug](https://github.com/carlosarraes/ccost/issues) • [💡 Request Feature](https://github.com/carlosarraes/ccost/issues)

</div>