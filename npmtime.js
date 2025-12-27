#!/usr/bin/env node

/**
 * npmtime.js - Time Machine for NPM packages.
 * Finds the version of npm packages that was available on a specific date.
 */

// Colors for better readability in the terminal
const GREEN = "\x1b[92m";
const RED = "\x1b[91m";
const YELLOW = "\x1b[93m";
const RESET = "\x1b[0m";

/**
 * Finds the latest version of a package that was published before the target date.
 * @param {string} packageName - The npm package name
 * @param {Date} targetDate - The cutoff date
 * @param {boolean} debug - Whether to show debug output
 * @returns {Promise<{version: string, date: Date} | {error: string}>}
 */
async function findVersionByDate(packageName, targetDate, debug = false) {
    const apiUrl = `https://registry.npmjs.org/${encodeURIComponent(packageName)}`;

    try {
        const response = await fetch(apiUrl, { timeout: 10000 });

        if (response.status === 404) {
            return { error: "Package not found on npm" };
        }

        if (!response.ok) {
            return { error: `HTTP error: ${response.status}` };
        }

        const data = await response.json();
        const times = data.time || {};
        const candidates = [];

        if (debug) {
            console.log(`\n[${packageName}] Analyzing version history...`);
        }

        // npm's "time" object contains version -> ISO date mappings
        for (const [version, timeStr] of Object.entries(times)) {
            // Skip special keys like "created" and "modified"
            if (version === "created" || version === "modified") {
                continue;
            }

            try {
                const publishDate = new Date(timeStr);

                if (publishDate <= targetDate) {
                    candidates.push({ version, date: publishDate });
                } else if (debug) {
                    console.log(`  - v${version}: Too recent (${publishDate.toISOString().split('T')[0]})`);
                }
            } catch (e) {
                // Skip versions with invalid dates
                continue;
            }
        }

        if (candidates.length === 0) {
            return { error: "No version found before the specified date." };
        }

        // Sort chronologically and get the last one (most recent before target date)
        candidates.sort((a, b) => a.date - b.date);
        return candidates[candidates.length - 1];

    } catch (e) {
        return { error: `Connection error: ${e.message}` };
    }
}

/**
 * Parses command line arguments.
 */
function parseArgs() {
    const args = process.argv.slice(2);

    if (args.length < 2) {
        console.log(`${YELLOW}Usage:${RESET} node npmtime.js <YYYY-MM-DD> <package1> [package2] ... [-v|--verbose]`);
        console.log(`\n${YELLOW}Example:${RESET}`);
        console.log(`  node npmtime.js 2020-01-15 express lodash axios`);
        console.log(`  node npmtime.js 2019-06-01 react react-dom -v`);
        process.exit(1);
    }

    const verbose = args.includes("-v") || args.includes("--verbose");
    const filteredArgs = args.filter(a => a !== "-v" && a !== "--verbose");

    const date = filteredArgs[0];
    const packages = filteredArgs.slice(1);

    return { date, packages, verbose };
}

/**
 * Validates and parses a date string in YYYY-MM-DD format.
 */
function parseDate(dateStr) {
    const regex = /^\d{4}-\d{2}-\d{2}$/;
    if (!regex.test(dateStr)) {
        return null;
    }

    const date = new Date(dateStr + "T23:59:59Z"); // End of day UTC
    if (isNaN(date.getTime())) {
        return null;
    }

    return date;
}

/**
 * Main entry point.
 */
async function main() {
    const { date: dateStr, packages, verbose } = parseArgs();

    const targetDate = parseDate(dateStr);
    if (!targetDate) {
        console.log(`${RED}Error: Invalid date. Use the format YYYY-MM-DD (e.g., 2019-12-12).${RESET}`);
        process.exit(1);
    }

    console.log(`--- Searching for versions up to ${YELLOW}${dateStr}${RESET} ---`);

    const installList = [];
    const errors = [];

    for (const pkg of packages) {
        const result = await findVersionByDate(pkg, targetDate, verbose);

        if (result.error) {
            errors.push(`${pkg}: ${result.error}`);
            console.log(`❌ ${RED}${pkg}${RESET}: ${result.error}`);
        } else {
            const specifier = `${pkg}@${result.version}`;
            installList.push(specifier);
            const dateFormatted = result.date.toISOString().split('T')[0];
            console.log(`✅ ${GREEN}${pkg}${RESET}: ${result.version} (from ${dateFormatted})`);
        }
    }

    console.log("-".repeat(60));

    if (installList.length > 0) {
        console.log("Copy and paste into your Dockerfile/package.json:");
        console.log(`\n${GREEN}npm install ${installList.join(" ")}${RESET}\n`);
    }

    if (errors.length > 0) {
        console.log(`${YELLOW}Attention to errors:${RESET}`);
        for (const err of errors) {
            console.log(` - ${err}`);
        }
    }
}

main();
