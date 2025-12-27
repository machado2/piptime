# piptime & npmtime

**Time Machine for PIP and NPM packages.**

Find the latest version of a package that was available before a specific date. Extremely useful for reproducing old environments or pinning dependencies for legacy projects.

## Tools

| Tool | Language | Registry |
|------|----------|----------|
| `piptime.py` | Python | [PyPI](https://pypi.org) |
| `npmtime.js` | Node.js | [npm](https://npmjs.com) |

## Features

-   Finds the latest version of a package released on or before a target date.
-   Supports multiple packages in a single run.
-   Outputs precise installation commands (e.g., `package==1.2.3` or `package@1.2.3`).
-   Ignores releases with no attached files (pip) or invalid dates.
-   Colored terminal output for better readability.

---

## piptime.py

### Prerequisites

-   Python 3.6+
-   `requests` library

### Installation

```bash
pip install requests
```

### Usage

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

### Output

```text
--- Searching for versions up to 2020-01-01 ---
✅ requests: 2.22.0 (from 2019-05-16)
------------------------------------------------------------
Copy and paste into your Dockerfile/requirements.txt:

pip install requests==2.22.0
```

---

## npmtime.js

### Prerequisites

-   Node.js 18+ (uses native `fetch`)

### Usage

```bash
node npmtime.js <DATE> <PACKAGE_1> <PACKAGE_2> ...
```

### Examples

**Find the version of `express` available on January 15th, 2020:**

```bash
node npmtime.js 2020-01-15 express
```

**Find versions for multiple packages:**

```bash
node npmtime.js 2019-06-01 react react-dom axios
```

**Verbose mode (shows version history analysis):**

```bash
node npmtime.js 2020-01-01 lodash -v
```

### Output

```text
--- Searching for versions up to 2020-01-15 ---
✅ express: 4.17.1 (from 2019-05-26)
------------------------------------------------------------
Copy and paste into your Dockerfile/package.json:

npm install express@4.17.1
```

---

## Use Cases

### Reproducing Old CI/CD Builds

Need to debug why a build from 2019 is failing? Find the exact dependency versions that were current at that time:

```bash
python piptime.py 2019-03-15 boto3 requests flask
node npmtime.js 2019-03-15 express lodash moment
```

### Legacy Project Maintenance

Working on an old project that needs specific dependency versions? Find compatible versions from when the project was active.

### Security Research

Investigating when a vulnerability was introduced? Find the package version that was available on a specific date.

---

## License

MIT
