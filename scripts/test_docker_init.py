#!/usr/bin/env python3
"""
Tests for docker-init.sh SQL parsing functionality.

These tests verify that the SQL statement splitting logic correctly handles:
- Multi-line statements
- Comments at the beginning and end of statements
- Nested parentheses
- Quotes (single and double)
- CHECK constraints with IN clauses
"""

import unittest
import os
import sys
import tempfile
import shutil

sys.path.insert(0, "/home/reinhard/jan-capstone")

from scripts.docker_init import split_sql_statements, strip_leading_comments


class TestStripLeadingComments(unittest.TestCase):
    """Test the strip_leading_comments function."""

    def test_no_comments(self):
        """Statement without comments should be unchanged."""
        sql = "SELECT * FROM users"
        result = strip_leading_comments(sql)
        self.assertEqual(result, "SELECT * FROM users")

    def test_single_line_comment(self):
        """Single line comment at start should be removed."""
        sql = "-- This is a comment\nSELECT * FROM users"
        result = strip_leading_comments(sql)
        self.assertEqual(result, "SELECT * FROM users")

    def test_multiple_comments(self):
        """Multiple comments at start should be removed."""
        sql = "-- Comment 1\n-- Comment 2\nSELECT * FROM users"
        result = strip_leading_comments(sql)
        self.assertEqual(result, "SELECT * FROM users")

    def test_comment_between_lines(self):
        """Comment in the middle should be preserved."""
        sql = "SELECT * -- comment\nFROM users"
        result = strip_leading_comments(sql)
        self.assertEqual(result, "SELECT * -- comment\nFROM users")

    def test_empty_after_stripping(self):
        """Statement that is only comments should become empty."""
        sql = "-- Just a comment"
        result = strip_leading_comments(sql)
        self.assertEqual(result, "")


class TestSplitSqlStatements(unittest.TestCase):
    """Test the split_sql_statements function."""

    def test_simple_select(self):
        """Simple SELECT statement should be split correctly."""
        content = "SELECT * FROM users;"
        statements = list(split_sql_statements(content))
        self.assertEqual(len(statements), 1)
        self.assertEqual(statements[0], "SELECT * FROM users;")

    def test_multiple_statements(self):
        """Multiple statements should all be split correctly."""
        content = "SELECT * FROM users; SELECT * FROM orders;"
        statements = list(split_sql_statements(content))
        self.assertEqual(len(statements), 2)
        self.assertEqual(statements[0], "SELECT * FROM users;")
        self.assertEqual(statements[1], "SELECT * FROM orders;")

    def test_multiline_statement(self):
        """Multi-line statement should be kept together."""
        content = """CREATE TABLE users (
  id INT PRIMARY KEY,
  name TEXT
);"""
        statements = list(split_sql_statements(content))
        self.assertEqual(len(statements), 1)
        self.assertIn("CREATE TABLE users", statements[0])

    def test_statement_with_check_constraint(self):
        """Statement with CHECK constraint should be split correctly."""
        content = "CREATE TABLE users (id INT, role TEXT CHECK (role IN ('A', 'B')));"
        statements = list(split_sql_statements(content))
        self.assertEqual(len(statements), 1)
        self.assertIn("CHECK", statements[0])

    def test_statement_with_nested_parens(self):
        """Statement with nested parentheses should be split correctly."""
        content = "SELECT * FROM users WHERE id IN (SELECT id FROM admins);"
        statements = list(split_sql_statements(content))
        self.assertEqual(len(statements), 1)
        self.assertIn("SELECT id FROM admins", statements[0])

    def test_quoted_semicolon(self):
        """Semicolon inside quotes should not split statement."""
        content = "INSERT INTO users VALUES ('test;name');"
        statements = list(split_sql_statements(content))
        self.assertEqual(len(statements), 1)
        self.assertIn("test;name", statements[0])

    def test_leading_comments(self):
        """Statement with leading comments should include comments."""
        content = "-- Comment\nSELECT * FROM users;"
        statements = list(split_sql_statements(content))
        self.assertEqual(len(statements), 1)
        self.assertIn("-- Comment", statements[0])
        self.assertIn("SELECT * FROM users", statements[0])


class TestIntegration(unittest.TestCase):
    """Integration tests using temporary SQL files."""

    def setUp(self):
        """Create a temporary directory for test files."""
        self.test_dir = tempfile.mkdtemp()

    def tearDown(self):
        """Clean up temporary directory."""
        shutil.rmtree(self.test_dir)

    def test_schema_with_check_constraints(self):
        """Test parsing a schema file with CHECK constraints."""
        schema_content = """-- Schema with CHECK
CREATE TABLE users (
  id INT PRIMARY KEY,
  role TEXT NOT NULL CHECK (role IN ('CUSTOMER', 'ORGANIZER'))
);

CREATE TABLE orders (
  id INT PRIMARY KEY,
  status TEXT CHECK (status IN ('PENDING', 'PAID'))
);"""

        filepath = os.path.join(self.test_dir, "schema.sql")
        with open(filepath, "w") as f:
            f.write(schema_content)

        with open(filepath) as f:
            content = f.read()

        statements = list(split_sql_statements(content))

        # Should have 2 CREATE TABLE statements (plus comments stripped)
        create_tables = [s for s in statements if "CREATE TABLE" in s]
        self.assertEqual(len(create_tables), 2)

        # Both should have CHECK constraints
        for stmt in create_tables:
            self.assertIn("CHECK", stmt)

    def test_seed_with_inserts(self):
        """Test parsing a seed file with INSERT statements."""
        seed_content = """-- Seed data
INSERT INTO users (id, name) VALUES ('usr_1', 'User One');
INSERT INTO users (id, name) VALUES ('usr_2', 'User Two');"""

        filepath = os.path.join(self.test_dir, "seed.sql")
        with open(filepath, "w") as f:
            f.write(seed_content)

        with open(filepath) as f:
            content = f.read()

        statements = list(split_sql_statements(content))

        # Should have 2 INSERT statements
        inserts = [s for s in statements if "INSERT INTO" in s]
        self.assertEqual(len(inserts), 2)

    def test_users_table_parsing(self):
        """Test that users table with CHECK IN is parsed correctly."""
        schema_content = """-- db/schema.sql
-- Idempotent schema for event ticketing system
-- Run this on a fresh database or one where tables may not exist

CREATE TABLE IF NOT EXISTS users (
  id            TEXT PRIMARY KEY,
  role          TEXT NOT NULL CHECK (role IN ('CUSTOMER', 'ORGANIZER')),
  email         TEXT NOT NULL UNIQUE,
  name          TEXT NOT NULL,
  avatar_url    TEXT,
  created_at    TEXT NOT NULL
);"""

        statements = list(split_sql_statements(schema_content))

        # Find the users table statement
        users_stmt = None
        for stmt in statements:
            if "CREATE TABLE IF NOT EXISTS users" in stmt:
                users_stmt = stmt
                break

        self.assertIsNotNone(users_stmt, "Users table statement not found")
        self.assertIn("CHECK", users_stmt)
        self.assertIn("IN", users_stmt)
        self.assertIn("'CUSTOMER'", users_stmt)
        self.assertIn("'ORGANIZER'", users_stmt)

    def test_seed_sql_with_users(self):
        """Test seed SQL with users table operations."""
        seed_content = """-- Insert users
INSERT INTO users (id, role, email, name, avatar_url, created_at) VALUES
  ('usr_org_1', 'ORGANIZER', 'organizer1@example.com', 'Organizer One', NULL, '2026-01-17T00:00:00Z');

INSERT INTO users (id, role, email, name, avatar_url, created_at) VALUES
  ('usr_cus_1', 'CUSTOMER', 'customer1@example.com', 'Customer One', NULL, '2026-01-17T00:00:00Z');"""

        statements = list(split_sql_statements(seed_content))

        # Should have 2 INSERT statements
        inserts = [s for s in statements if "INSERT INTO users" in s]
        self.assertEqual(len(inserts), 2)

        # Both should have VALUES
        for stmt in inserts:
            self.assertIn("VALUES", stmt)


if __name__ == "__main__":
    unittest.main()
