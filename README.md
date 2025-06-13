# ccost - Claude Cost Tracking Tool

<div align="center">

**Accurate Claude API usage tracking with intelligent deduplication**

[![GitHub Release](https://img.shields.io/github/v/release/carlosarraes/ccost?style=flat&color=blue)](https://github.com/carlosarraes/ccost/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)

</div>

## 🎯 What is ccost?

ccost is a comprehensive Claude API usage tracking and cost analysis tool designed to provide accurate insights into your AI usage patterns. It features intelligent message deduplication to prevent inflated cost calculations when working with branched conversations.

### ✅ Key Features:
- ✅ **Intelligent deduplication** using UUID+RequestID hashing to ensure accurate cost calculations
- ✅ **Multi-currency support** with live exchange rates (EUR, GBP, JPY, CNY, BRL, etc.)
- ✅ **Model switching tracking** to monitor changes within conversations
- ✅ **SQLite caching** for offline operation and improved performance
- ✅ **Timezone-aware** daily cutoffs and filtering
- ✅ **Project-based analysis** to track usage across different work contexts
- ✅ **Comprehensive filtering** by date ranges, models, and projects

## 🚀 Quick Start

### One-Line Installation

```bash
curl -sSf https://raw.githubusercontent.com/carlosarraes/ccost/main/install.sh | sh
```

### Basic Usage

```bash
# View today's usage
ccost usage today

# View this week's usage in EUR
ccost usage this-week --currency EUR

# Analyze projects by cost
ccost projects cost

# View detailed daily breakdown
ccost usage daily --days 7
```

## 📊 Features

### 🔍 Usage Analysis
- **Timeframe commands**: `today`, `yesterday`, `this-week`, `this-month`, `daily`
- **Project filtering**: `--project myproject`
- **Model filtering**: `--model claude-sonnet-4`
- **Date ranges**: `--since 2025-01-01 --until 2025-01-31`
- **Deduplication statistics**: See exactly how many duplicate messages were filtered

### 💰 Multi-Currency Support
- **Real-time conversion** via European Central Bank API
- **Cached rates** for offline usage (24-hour TTL)
- **Supported currencies**: USD, EUR, GBP, JPY, CNY, BRL, and more
- **Proper formatting**: $12.34, €10.45, £8.99, ¥1,234

### 📈 Project Analysis
- **Smart project detection** from file paths and `cwd` fields
- **Sorting options**: by name, cost, or token usage
- **Usage statistics**: total tokens, costs, and model distribution
- **Project comparison**: identify your most active projects

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
```

## 📋 Command Reference

### Usage Commands
```bash
# Basic usage analysis
ccost usage                           # Overall usage summary
ccost usage today                     # Today's usage
ccost usage yesterday                 # Yesterday's usage  
ccost usage this-week                 # This week's usage
ccost usage this-month                # This month's usage
ccost usage daily                     # Daily breakdown (7 days)
ccost usage daily --days 30           # Daily breakdown (30 days)

# With filters and options
ccost usage today --project myproj    # Filter by project
ccost usage today --model sonnet-4    # Filter by model
ccost usage --since 2025-01-01        # Custom date range
ccost usage --currency EUR            # Convert to EUR
ccost usage --json                    # JSON output
ccost usage --verbose                 # Detailed statistics
```

### Project Analysis
```bash
ccost projects                        # List all projects
ccost projects cost                   # Sort by cost (highest first)
ccost projects tokens                 # Sort by token usage
ccost projects name                   # Sort alphabetically
```

### Pricing Management
```bash
ccost pricing list                    # Show current model pricing
ccost pricing set claude-4 12.0 36.0  # Set custom pricing (input/output per 1M tokens)
```

### Configuration
```bash
ccost config show                     # Display current configuration
ccost config init                     # Create fresh config file
ccost config set key value            # Set configuration value
```

## 🔧 Configuration

ccost stores configuration at `~/.config/ccost/config.toml`:

```toml
[currency]
default_currency = "USD"

[timezone]
timezone = "UTC"
daily_cutoff_hour = 0

[output]
colored = true
decimal_places = 2
date_format = "yyyy-mm-dd"  # Options: "yyyy-mm-dd", "dd-mm-yyyy", "mm-dd-yyyy"

[cache]
exchange_rate_ttl_hours = 24
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

### Usage Summary
```
 Project          Input Tokens   Output Tokens   Cache Creation   Cache Read      Messages   Total Cost 
────────────────────────────────────────────────────────────────────────────────────────────────────────
 project1                1,565          17,311          817,314       6,153,529        258        $4.31 
 project2               23,159         395,272        5,536,364     108,290,250      1,409       $80.74 
 project3                    4              36           30,636               0          1        $0.12 
 project4               28,597          95,017        4,242,745      78,565,562      1,127       $34.64 
 project5                1,212         138,925        2,154,078      40,701,941        511       $22.38 
 project6                  349          26,853          444,059       2,689,493         69       $13.09 
 project7              259,865         358,885       23,235,487     417,878,538      5,343      $155.54 
 project8               54,976       1,176,918        8,527,443     270,777,448      3,233      $129.54 
────────────────────────────────────────────────────────────────────────────────────────────────────────
 TOTAL                 369,727       2,209,217       45,987,126     925,289,761     11,951     $440.36
```

### Today's Usage
```
 Project        Input Tokens   Output Tokens   Cache Creation   Cache Read   Messages   Total Cost 
───────────────────────────────────────────────────────────────────────────────────────────────────
 project1               13,713          26,680          920,900   31,156,134        423       $10.06 
 project2                    9               2            3,616       23,275          2        $0.01 
 project3                   24             631           32,955       86,832          4        $0.23 
 project4                    4               1           92,238            0          1        $0.03 
───────────────────────────────────────────────────────────────────────────────────────────────────
 TOTAL                  13,750          27,314        1,049,709   31,266,241        430       $10.33
```

### Daily Breakdown (Last 7 Days)
```
 Date         Input Tokens   Output Tokens   Cache Creation   Cache Read    Messages   Projects   Total Cost 
─────────────────────────────────────────────────────────────────────────────────────────────────────────────
 2025-06-07         12,718         180,396        9,848,514   162,746,099      2,290          5       $61.31 
 2025-06-08          4,350         100,533        5,214,948    58,807,114        983          4       $31.43 
 2025-06-09         45,010         166,494        7,060,570   162,008,012      1,971          3       $63.60 
 2025-06-10         31,231          48,010        3,502,083    88,633,343      1,030          4       $33.49 
 2025-06-11        178,045         173,676        7,799,069   178,055,996      2,271          5       $64.29 
 2025-06-12         45,106          72,329        9,296,709    89,519,256      1,187          1       $30.87 
 2025-06-13         13,750          27,314        1,049,709    31,266,241        430          4       $10.33 
─────────────────────────────────────────────────────────────────────────────────────────────────────────────
 TOTAL             330,210         768,752       43,771,602   771,036,061     10,162         26      $295.31
```

## 🏗️ Architecture

ccost is built with a robust, modular architecture:

- **Parser Module**: JSONL parsing with full Claude data structure support
- **Deduplication Engine**: SHA256-based message deduplication using UUID+RequestID
- **Database Layer**: SQLite with WAL mode for persistence and caching
- **Currency Manager**: ECB API integration with automatic caching
- **Analysis Engine**: Usage tracking, project analysis, and cost calculation
- **CLI Framework**: Comprehensive command structure with clap

### Data Flow
1. **Parse** JSONL files from `~/.claude/projects/`
2. **Deduplicate** messages using intelligent hash fallback strategy
3. **Analyze** usage patterns and calculate costs
4. **Cache** results in SQLite for performance
5. **Display** results with professional formatting

## 🔍 Deduplication Strategy

ccost uses a sophisticated multi-tier fallback strategy for message deduplication:

1. **Priority 1**: `uuid + request_id` (most reliable when both available)
2. **Priority 2**: `uuid + message.id` (common when request_id is null)  
3. **Priority 3**: `message.id` only (last resort for messages without uuid)
4. **Priority 4**: `uuid` only (legacy support)

This ensures maximum accuracy while maintaining compatibility with all Claude data formats.

### Deduplication Statistics
ccost provides detailed deduplication reporting:
- **Total messages found**: Raw count from JSONL files
- **Duplicates removed**: Number of duplicate messages filtered
- **Deduplication rate**: Percentage of duplicates (typically 12-18%)
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