# pkgtime

**Universal Time Machine for Package Managers.**

`pkgtime` is a single, unified CLI tool written in Rust that finds the latest version of a package available before a specific date. It is the successor to `piptime.py` and `npmtime.js`.

## Supported Managers

| Manager | CLI Arg | Registry |
|---------|---------|----------|
| **Pip** | `pip` | [PyPI](https://pypi.org) |
| **Npm** | `npm` | [npm](https://npmjs.com) |
| **Cargo** | `cargo` | [Crates.io](https://crates.io) |
| **Gem** | `gem` | [RubyGems](https://rubygems.org) |
| **Composer** | `composer` | [Packagist](https://packagist.org) |

## Installation

### From Source

Ensure you have Rust/Cargo installed.

```bash
cargo install --path .
```

This will install the `pkgtime` binary to your Cargo bin path.

## Usage

```bash
pkgtime <MANAGER> <DATE> <PACKAGES...> [OPTIONS]
```

### Arguments

- `MANAGER`: The package manager to target. Values: `pip`, `npm`, `cargo`, `gem`.
- `DATE`: Cutoff date in `YYYY-MM-DD` format.
- `PACKAGES`: Space-separated list of packages to check.

### Options

- `-v, --verbose`: Enable verbose output (shows HTTP requests and rejected versions).
- `-h, --help`: Show help information.

## Examples

### Python (pip)
Find `requests` version from Jan 1st, 2020:
```bash
pkgtime pip 2020-01-01 requests
# Output: pip install requests==2.22.0
```

### Node.js (npm)
Find versions for `express` and `lodash`:
```bash
pkgtime npm 2020-01-15 express lodash
# Output: npm install express@4.17.1 lodash@4.17.15
```

### Rust (cargo)
Find `serde` version for 2021:
```bash
pkgtime cargo 2021-06-01 serde
# Output: serde = "=1.0.126"
```

### Ruby (gem)
Find `rails` version from 2015:
```bash
pkgtime gem 2015-01-01 rails
# Output: gem 'rails', '4.2.0'
```

### PHP (Composer)
Find `monolog` version from 2021:
```bash
pkgtime composer 2021-01-01 monolog/monolog
# Output: composer require monolog/monolog:2.2.0
```

---

## Legacy Scripts

This repository also contains the original prototype scripts:
- `piptime.py` (Python)
- `npmtime.js` (Node.js)

These are kept for reference but `pkgtime` is the recommended tool.

## License

MIT
