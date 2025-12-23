# piptime.py

**Time Machine for PIP packages.**

`piptime.py` helps you find the latest version of a Python package that was available before a specific date. This is extremely useful for reproducing old environments or pinning dependencies for legacy projects.

## Features

-   Finds the latest version of a package released on or before a target date.
-   Supports multiple packages in a single run.
-   Outputs precise installation commands (e.g., `package==1.2.3`).
-   Ignores releases with no attached files (e.g., broken releases).
-   Colors for better readability in the terminal.

## Prerequisites

-   Python 3.6+
-   `requests` library

## Installation

1.  Clone this repository or download `piptime.py`.
2.  Install the dependencies:

    ```bash
    pip install requests
    ```

## Usage

Run the script providing a date (YYYY-MM-DD) and a list of packages.

```bash
python piptime.py <DATE> <PACKAGE_1> <PACKAGE_2> ...
```

### Examples

**Find the version of `requests` available on January 1st, 2020:**

```bash
python piptime.py 2020-01-01 requests
```

**Find versions for multiple packages:**

```bash
python piptime.py 2022-06-15 numpy pandas scipy
```

**Verbose mode (shows version history analysis):**

```bash
python piptime.py 2021-01-01 flask -v
```

## Output

The script outputs the package version and the release date found.

```text
--- Searching for versions up to 2020-01-01 ---
 -> Checking requests... ✅ requests: 2.22.0 (from 2019-05-16)

------------------------------------------------------------
Copy and paste into your Dockerfile/requirements.txt:

pip install requests==2.22.0
```

## License

MIT
