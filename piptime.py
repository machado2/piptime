import requests
import argparse
from datetime import datetime
import sys

# Colors for better readability in the terminal
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
RESET = "\033[0m"

def find_version_by_date(package_name, target_date, debug=False):
    api_url = f"https://pypi.org/pypi/{package_name}/json"
    
    try:
        response = requests.get(api_url, timeout=10)
        if response.status_code == 404:
            return None, "Package not found on PyPI"
        response.raise_for_status()
        data = response.json()
    except Exception as e:
        return None, f"Connection error: {str(e)}"

    releases = data.get("releases", {})
    candidates = []

    if debug:
        print(f"\n[{package_name}] Analyzing version history...")

    for version, files in releases.items():
        if not files:
            if debug: print(f"  - v{version}: Ignored (no files attached)")
            continue
        
        try:
            # Get the date of the first file (usually .tar.gz or .whl)
            upload_time_str = files[0]['upload_time']
            upload_time = datetime.strptime(upload_time_str, "%Y-%m-%dT%H:%M:%S")
            
            if upload_time <= target_date:
                candidates.append((version, upload_time))
            elif debug:
                print(f"  - v{version}: Too recent ({upload_time.date()})")
                
        except (ValueError, IndexError):
            continue

    if not candidates:
        return None, "No version found before the specified date."

    # Sort chronologically and get the last one
    candidates.sort(key=lambda x: x[1])
    
    # Return the champion: ((version, date), None)
    return candidates[-1], None

def main():
    parser = argparse.ArgumentParser(description='Time Machine for PIP packages.')
    parser.add_argument('date', help='Cutoff date (YYYY-MM-DD)')
    parser.add_argument('packages', nargs='+', help='List of packages')
    parser.add_argument('-v', '--verbose', action='store_true', help='Show search details')

    args = parser.parse_args()

    try:
        target_date = datetime.strptime(args.date, "%Y-%m-%d")
        # Set time to the end of the day to include releases released on the day itself
        target_date = target_date.replace(hour=23, minute=59, second=59)
    except ValueError:
        print(f"{RED}Error: Invalid date. Use the format YYYY-MM-DD (e.g., 2019-12-12).{RESET}")
        sys.exit(1)

    print(f"--- Searching for versions up to {YELLOW}{target_date.date()}{RESET} ---")
    
    install_list = []
    errors = []

    for pkg in args.packages:
        print(f" -> Checking {pkg}...", end=" ", flush=True)
        
        # The call is now safe and standardized
        result, error_msg = find_version_by_date(pkg, target_date, args.verbose)
        
        if result:
            version, r_date = result
            # Build the version string (e.g., codecov==2.0.15)
            specifier = f"{pkg}=={version}"
            install_list.append(specifier)
            print(f"\r✅ {GREEN}{pkg}{RESET}: {version} (from {r_date.date()}){' '*20}")
        else:
            errors.append(f"{pkg}: {error_msg}")
            print(f"\r❌ {RED}{pkg}{RESET}: {error_msg}{' '*20}")

    print("-" * 60)
    
    if install_list:
        print("Copy and paste into your Dockerfile/requirements.txt:")
        print(f"\n{GREEN}pip install {' '.join(install_list)}{RESET}\n")
    
    if errors:
        print(f"{YELLOW}Attention to errors:{RESET}")
        for err in errors:
            print(f" - {err}")

if __name__ == "__main__":
    main()
