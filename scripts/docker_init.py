#!/usr/bin/env python3
"""SQL parsing utilities for docker-init.sh.

This module provides functions for splitting SQL statements and
executing them against the RDBMS API.
"""

import sys
import urllib.request
import json
import os

API_URL = os.environ.get("API_URL", "http://localhost:8080")
DB_DIR = os.environ.get("DB_DIR", "/db")
MAX_RETRIES = int(os.environ.get("MAX_RETRIES", "30"))
RETRY_INTERVAL = int(os.environ.get("RETRY_INTERVAL", "2"))


def strip_leading_comments(sql):
    """Remove SQL comments from the beginning of a SQL string.

    Args:
        sql: A SQL statement string, possibly with leading comments.

    Returns:
        The SQL string with leading comments removed.
    """
    lines = sql.split("\n")
    result_lines = []
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("--"):
            continue
        result_lines.append(line)
    return "\n".join(result_lines)


def split_sql_statements(content):
    """Split SQL content into individual statements.

    Handles:
    - Multi-line statements
    - Nested parentheses
    - Quotes (single and double)
    - Escaped single quotes (SQL uses '' to escape)
    - Comments within statements

    Args:
        content: A string containing SQL statements separated by semicolons.

    Yields:
        Individual SQL statements (including leading comments).
    """
    statement = ""
    in_quote = None
    paren_depth = 0
    i = 0

    while i < len(content):
        char = content[i]

        if in_quote is None:
            if char in ("'", '"'):
                in_quote = char
            elif char == "(":
                paren_depth += 1
            elif char == ")":
                if paren_depth > 0:
                    paren_depth -= 1
        elif char == in_quote:
            in_quote = None
        elif char == "'" and in_quote == "'":
            if i + 1 < len(content) and content[i + 1] == "'":
                i += 1

        statement += char

        if char == ";" and paren_depth == 0 and in_quote is None:
            yield statement.strip()
            statement = ""

        i += 1

    if statement.strip():
        yield statement.strip()


def execute_statement(stmt, api_url=None):
    """Execute a single SQL statement against the API.

    Args:
        stmt: The SQL statement to execute.
        api_url: Optional API URL override.

    Returns:
        The JSON response from the API.

    Raises:
        Exception: If the request fails.
    """
    stmt = stmt.strip()
    if not stmt:
        return None
    stmt = strip_leading_comments(stmt)
    stmt = stmt.strip()
    if not stmt:
        return None
    stmt = stmt.replace("\n", " ").replace("\r", " ")
    while "  " in stmt:
        stmt = stmt.replace("  ", " ")

    url = api_url or API_URL
    data = json.dumps({"sql": stmt}).encode("utf-8")
    req = urllib.request.Request(
        f"{url}/api/sql",
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=2) as response:
        return json.loads(response.read().decode("utf-8"))


def wait_for_api(api_url=None, max_retries=None, retry_interval=None):
    """Wait for the API to become available.

    Args:
        api_url: Optional API URL override.
        max_retries: Optional max retries override.
        retry_interval: Optional retry interval override.

    Returns:
        True if API is available, raises SystemExit otherwise.
    """
    url = api_url or API_URL
    retries = max_retries if max_retries is not None else MAX_RETRIES
    interval = retry_interval if retry_interval is not None else RETRY_INTERVAL

    print(f"Waiting for backend-service at {url}...")
    import time

    for attempt in range(retries):
        try:
            req = urllib.request.Request(f"{url}/api/health")
            with urllib.request.urlopen(req, timeout=2) as response:
                if response.status == 200:
                    print("Backend-service is ready!")
                    return True
        except Exception:
            pass
        print(f"Attempt {attempt + 1}/{retries} - waiting {interval}s...")
        time.sleep(interval)
    print("Error: Backend-service did not become ready in time")
    sys.exit(1)


def execute_sql_file(filepath, api_url=None):
    """Execute all SQL statements in a file.

    Args:
        filepath: Path to the SQL file.
        api_url: Optional API URL override.

    Returns:
        List of results from executing each statement.
    """
    filename = os.path.basename(filepath)
    if not os.path.isfile(filepath):
        print(f"Warning: File {filepath} not found, skipping")
        return []

    print(f"Executing {filename}...")

    results = []
    with open(filepath) as f:
        content = f.read()

    for stmt in split_sql_statements(content):
        result = execute_statement(stmt, api_url)
        results.append(result)

    print(f"{filename} execution complete")
    return results


def main():
    """Main entry point for docker-init.sh."""
    wait_for_api()

    if os.path.isdir(DB_DIR):
        sql_files = sorted(
            os.path.join(DB_DIR, f) for f in os.listdir(DB_DIR) if f.endswith(".sql")
        )
        for sql_file in sql_files:
            execute_sql_file(sql_file)
    else:
        print(f"Warning: Directory {DB_DIR} not found, skipping initialization")

    print("Database initialization complete!")


if __name__ == "__main__":
    main()
