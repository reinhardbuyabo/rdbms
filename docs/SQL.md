# SQL Syntax Reference

This document describes the SQL syntax supported by the Eventify RDBMS.

## Data Types

| Data Type | Description |
|-----------|-------------|
| `INT` | 64-bit signed integer |
| `INTEGER` | Alias for INT |
| `TEXT` | Variable-length text string |
| `REAL` | 64-bit floating point |
| `FLOAT` | Alias for REAL |
| `BOOLEAN` | Boolean value (TRUE/FALSE) |
| `BLOB` | Binary large object (byte array) |

## Data Definition (DDL)

### CREATE TABLE

```sql
CREATE TABLE table_name (
    column_name data_type [constraints],
    ...
);
```

**Constraints:**
- `PRIMARY KEY` - Column is the primary key
- `UNIQUE` - All values must be distinct
- `NOT NULL` - Column cannot contain NULL values
- `DEFAULT value` - Default value for the column

**Examples:**

```sql
-- Basic table
CREATE TABLE users (
    id INT PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    age INT,
    is_active BOOLEAN DEFAULT TRUE
);

-- Table with blob
CREATE TABLE files (
    id INT PRIMARY KEY,
    name TEXT,
    payload BLOB
);

-- Table with default values
CREATE TABLE products (
    id INT PRIMARY KEY,
    name TEXT NOT NULL,
    price REAL DEFAULT 0.0,
    stock INT DEFAULT 0
);
```

### ALTER TABLE

```sql
-- Add a column
ALTER TABLE table_name ADD COLUMN column_name data_type [constraints];

-- Rename the table
ALTER TABLE table_name RENAME TO new_table_name;

-- Rename a column
ALTER TABLE table_name RENAME COLUMN column_name TO new_column_name;

-- Drop a column
ALTER TABLE table_name DROP COLUMN column_name;
```

**Examples:**

```sql
ALTER TABLE users ADD COLUMN phone TEXT;
ALTER TABLE users RENAME TO customers;
ALTER TABLE users RENAME COLUMN name TO full_name;
ALTER TABLE users DROP COLUMN age;
```

**Note:** Only one ALTER TABLE operation is supported per statement.

### DROP TABLE

```sql
DROP TABLE table_name;
DROP TABLE IF EXISTS table_name;
```

**Examples:**

```sql
DROP TABLE users;
DROP TABLE IF EXISTS old_table;
```

### CREATE INDEX

```sql
CREATE INDEX index_name ON table_name (column_name);
CREATE UNIQUE INDEX index_name ON table_name (column_name);
CREATE INDEX IF NOT EXISTS index_name ON table_name (column_name);
```

**Examples:**

```sql
CREATE INDEX idx_users_email ON users(email);
CREATE UNIQUE INDEX idx_products_sku ON products(sku);
CREATE INDEX IF NOT EXISTS idx_orders_user ON orders(user_id);
```

## Data Manipulation (DML)

### INSERT

```sql
INSERT INTO table_name VALUES (value1, value2, ...);
INSERT INTO table_name (col1, col2) VALUES (v1, v2);
INSERT INTO table_name DEFAULT VALUES;
```

**Examples:**

```sql
INSERT INTO users VALUES (1, 'Alice', 'alice@example.com');
INSERT INTO users (id, name, email) VALUES (2, 'Bob', 'bob@example.com');
INSERT INTO products (name, price) VALUES ('Widget', 9.99);
INSERT INTO users DEFAULT VALUES;
```

### UPDATE

```sql
UPDATE table_name SET col1 = value1, col2 = value2 WHERE condition;
```

**Examples:**

```sql
UPDATE users SET name = 'Alice Smith' WHERE id = 1;
UPDATE products SET price = price * 1.1 WHERE category = 'Electronics';
UPDATE users SET age = 30 WHERE name = 'Bob' AND email = 'bob@example.com';
```

**Note:** UPDATE only supports simple table references (no JOINs in UPDATE).

### DELETE

```sql
DELETE FROM table_name WHERE condition;
```

**Examples:**

```sql
DELETE FROM users WHERE id = 1;
DELETE FROM products WHERE stock = 0;
DELETE FROM orders WHERE status = 'cancelled';
```

**Note:** DELETE only supports single table (no JOINs in DELETE).

## Querying (DQL)

### SELECT

```sql
SELECT * FROM table_name;
SELECT column1, column2 FROM table_name;
SELECT * FROM table_name WHERE condition;
SELECT * FROM table1 JOIN table2 ON table1.col = table2.col;
```

**Clause Order:**
1. SELECT
2. FROM
3. JOIN
4. WHERE
5. GROUP BY
6. HAVING
7. ORDER BY
8. LIMIT/OFFSET

### WHERE Clause

```sql
SELECT * FROM users WHERE age > 18;
SELECT * FROM products WHERE price BETWEEN 10 AND 100;
SELECT * FROM users WHERE name LIKE 'A%';
SELECT * FROM users WHERE email IN ('a@b.com', 'c@d.com');
```

**Operators:**
- `=`, `!=`, `<>`, `<`, `<=`, `>`, `>=` - Comparison
- `AND`, `OR`, `NOT` - Logical
- `BETWEEN ... AND ...` - Range
- `LIKE` - Pattern matching
- `IN` - Membership
- `IS NULL`, `IS NOT NULL` - Null checks

### JOIN

```sql
SELECT * FROM table1 INNER JOIN table2 ON table1.id = table2.table1_id;
SELECT * FROM table1 LEFT OUTER JOIN table2 ON table1.id = table2.table1_id;
SELECT * FROM table1 RIGHT OUTER JOIN table2 ON table1.id = table2.table1_id;
SELECT * FROM table1 FULL OUTER JOIN table2 ON table1.id = table2.table1_id;
SELECT * FROM table1 CROSS JOIN table2;
```

**Examples:**

```sql
SELECT u.name, o.id FROM users u JOIN orders o ON u.id = o.user_id;
SELECT * FROM Event e LEFT JOIN TicketType t ON e.id = t.event_id;
```

### GROUP BY and Aggregates

```sql
SELECT column, COUNT(*) FROM table GROUP BY column;
SELECT department, AVG(salary), SUM(salary) FROM employees GROUP BY department;
SELECT status, COUNT(*) FROM orders GROUP BY status HAVING COUNT(*) > 10;
```

**Aggregate Functions:**
- `COUNT(*)` or `COUNT(column)` - Count rows
- `SUM(column)` - Sum of values
- `AVG(column)` - Average of values
- `MIN(column)` - Minimum value
- `MAX(column)` - Maximum value

### ORDER BY

```sql
SELECT * FROM users ORDER BY name;
SELECT * FROM products ORDER BY price DESC;
SELECT * FROM orders ORDER BY created_at ASC, status DESC;
```

### LIMIT and OFFSET

```sql
SELECT * FROM users LIMIT 10;
SELECT * FROM users LIMIT 10 OFFSET 20;
SELECT * FROM users ORDER BY id LIMIT 5 OFFSET 15;
```

### Expressions

```sql
-- Arithmetic
SELECT price * quantity FROM orders;
SELECT (price * 0.1) AS discount FROM products;

-- String concatenation (|| operator)
SELECT first_name || ' ' || last_name AS full_name FROM users;

-- Type casting
SELECT CAST(price AS INTEGER) FROM products;
SELECT CAST(id AS TEXT) FROM users;

-- Boolean literals
SELECT * FROM products WHERE is_active = TRUE;
SELECT * FROM users WHERE verified = FALSE;

-- Constants
SELECT 'Hello World' AS greeting;
SELECT 42 AS answer;
SELECT 3.14 AS pi;
SELECT TRUE AS is_valid, FALSE AS is_deleted;
```

### Subqueries

```sql
SELECT * FROM (SELECT id FROM users) AS sub_u WHERE id > 5;
SELECT * FROM users WHERE id IN (SELECT user_id FROM orders);
```

## Transactions

```sql
BEGIN;
-- SQL statements
COMMIT;

-- Or rollback
ROLLBACK;
```

**Example:**

```sql
BEGIN;
INSERT INTO accounts (id, balance) VALUES (1, 1000);
INSERT INTO accounts (id, balance) VALUES (2, 500);
UPDATE accounts SET balance = balance - 100 WHERE id = 1;
UPDATE accounts SET balance = balance + 100 WHERE id = 2;
COMMIT;
```

## Unsupported Features

The following SQL features are **not yet supported**:

- `UNION`, `INTERSECT`, `EXCEPT`
- `INSERT ... SELECT`
- Subqueries in WHERE clause (limited support)
- Window functions
- Common table expressions (CTE / WITH)
- Views
- Triggers
- Stored procedures
- Multiple ALTER TABLE operations in one statement
- Foreign keys and referential integrity
- CHECK constraints
- Transactions with savepoints
- Partial indexes
- Indexes on expressions
- FULL TEXT search
- JSON data type and operations

## Reserved Keywords

The following keywords are reserved and cannot be used as identifiers without quoting:

- `SELECT`, `FROM`, `WHERE`, `AND`, `OR`, `NOT`
- `INSERT`, `UPDATE`, `DELETE`, `SET`
- `CREATE`, `TABLE`, `INDEX`, `DROP`, `ALTER`
- `PRIMARY`, `KEY`, `FOREIGN`, `REFERENCES`
- `UNIQUE`, `NOT`, `NULL`, `DEFAULT`
- `JOIN`, `INNER`, `LEFT`, `RIGHT`, `FULL`, `OUTER`, `ON`
- `GROUP`, `BY`, `HAVING`, `ORDER`, `ASC`, `DESC`
- `LIMIT`, `OFFSET`
- `AS`, `DISTINCT`, `ALL`
- `BETWEEN`, `IN`, `IS`, `LIKE`, `IN`
- `CASE`, `WHEN`, `THEN`, `ELSE`, `END`
- `CAST`, `NULLIF`, `COALESCE`
- `BEGIN`, `COMMIT`, `ROLLBACK`
- `TRUE`, `FALSE`
- `INT`, `INTEGER`, `TEXT`, `REAL`, `FLOAT`, `BOOLEAN`, `BLOB`
- `PRAGMA`, `EXPLAIN`, `VACUUM`, `ANALYZE`

## Examples

### Create a Complete Schema

```sql
-- Users table
CREATE TABLE users (
    id INT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    name TEXT,
    role TEXT DEFAULT 'CUSTOMER',
    created_at TEXT,
    updated_at TEXT
);

-- Events table
CREATE TABLE events (
    id INT PRIMARY KEY,
    organizer_id INT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    location TEXT,
    start_time TEXT,
    end_time TEXT,
    status TEXT DEFAULT 'DRAFT',
    created_at TEXT,
    updated_at TEXT
);

-- Ticket types table
CREATE TABLE ticket_types (
    id INT PRIMARY KEY,
    event_id INT NOT NULL,
    name TEXT NOT NULL,
    price REAL DEFAULT 0,
    capacity INT NOT NULL,
    sales_start TEXT,
    sales_end TEXT,
    created_at TEXT,
    updated_at TEXT
);

-- Orders table
CREATE TABLE orders (
    id INT PRIMARY KEY,
    user_id INT NOT NULL,
    event_id INT NOT NULL,
    status TEXT DEFAULT 'PENDING',
    total_amount REAL,
    created_at TEXT,
    updated_at TEXT
);

-- Create indexes
CREATE INDEX idx_events_organizer ON events(organizer_id);
CREATE INDEX idx_ticket_types_event ON ticket_types(event_id);
CREATE INDEX idx_orders_user ON orders(user_id);
CREATE INDEX idx_orders_event ON orders(event_id);
```

### Insert Sample Data

```sql
INSERT INTO users (id, email, name, role) VALUES
(1, 'organizer@example.com', 'Event Organizer', 'ORGANIZER'),
(2, 'customer@example.com', 'Regular Customer', 'CUSTOMER');

INSERT INTO events (id, organizer_id, name, status) VALUES
(1, 1, 'Tech Conference 2024', 'PUBLISHED'),
(2, 1, 'Music Festival', 'DRAFT');

INSERT INTO ticket_types (id, event_id, name, price, capacity) VALUES
(1, 1, 'Early Bird', 99.00, 100),
(2, 1, 'Regular', 149.00, 200),
(3, 2, 'General Admission', 75.00, 500);

INSERT INTO orders (id, user_id, event_id, status, total_amount) VALUES
(1, 2, 1, 'PAID', 149.00);
```

### Query Data

```sql
-- List all published events with ticket types
SELECT e.*, t.name as ticket_name, t.price, t.capacity
FROM events e
LEFT JOIN ticket_types t ON e.id = t.event_id
WHERE e.status = 'PUBLISHED';

-- Get order summary for an event
SELECT 
    e.name as event_name,
    COUNT(o.id) as total_orders,
    SUM(o.total_amount) as total_revenue
FROM events e
LEFT JOIN orders o ON e.id = o.event_id
WHERE o.status = 'PAID'
GROUP BY e.id, e.name;

-- Find customers with most orders
SELECT 
    u.name,
    u.email,
    COUNT(o.id) as order_count,
    SUM(o.total_amount) as total_spent
FROM users u
JOIN orders o ON u.id = o.user_id
WHERE o.status = 'PAID'
GROUP BY u.id, u.name, u.email
ORDER BY total_spent DESC
LIMIT 10;
```

### Update and Cleanup

```sql
-- Update event status
UPDATE events SET status = 'PUBLISHED' WHERE id = 1;

-- Cancel orders older than 30 days
UPDATE orders SET status = 'CANCELLED' 
WHERE status = 'PENDING' 
AND created_at < datetime('now', '-30 days');

-- Remove cancelled orders
DELETE FROM orders WHERE status = 'CANCELLED';
```
