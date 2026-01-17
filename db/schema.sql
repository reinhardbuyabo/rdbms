-- db/schema.sql
-- Idempotent schema for event ticketing system
-- Run this on a fresh database or one where tables may not exist

CREATE TABLE IF NOT EXISTS users (
  id            TEXT PRIMARY KEY,
  role          TEXT NOT NULL CHECK (role IN ('CUSTOMER', 'ORGANIZER')),
  email         TEXT NOT NULL UNIQUE,
  name          TEXT NOT NULL,
  avatar_url    TEXT,
  created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS events (
  id            TEXT PRIMARY KEY,
  organizer_id  TEXT NOT NULL,
  title         TEXT NOT NULL,
  description   TEXT,
  location      TEXT,
  starts_at     TEXT NOT NULL,
  ends_at       TEXT,
  published     INTEGER NOT NULL DEFAULT 0,
  created_at    TEXT NOT NULL,

  FOREIGN KEY (organizer_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS ticket_types (
  id            TEXT PRIMARY KEY,
  event_id      TEXT NOT NULL,
  name          TEXT NOT NULL,
  price_cents   INTEGER NOT NULL CHECK (price_cents >= 0),
  capacity      INTEGER NOT NULL CHECK (capacity >= 0),
  created_at    TEXT NOT NULL,

  FOREIGN KEY (event_id) REFERENCES events(id),
  UNIQUE(event_id, name)
);

CREATE TABLE IF NOT EXISTS orders (
  id            TEXT PRIMARY KEY,
  customer_id   TEXT NOT NULL,
  event_id      TEXT NOT NULL,
  status        TEXT NOT NULL CHECK (status IN ('PENDING', 'PAID', 'CANCELLED')),
  total_cents   INTEGER NOT NULL CHECK (total_cents >= 0),
  created_at    TEXT NOT NULL,

  FOREIGN KEY (customer_id) REFERENCES users(id),
  FOREIGN KEY (event_id) REFERENCES events(id)
);

CREATE TABLE IF NOT EXISTS tickets (
  id             TEXT PRIMARY KEY,
  order_id       TEXT NOT NULL,
  ticket_type_id TEXT NOT NULL,
  event_id       TEXT NOT NULL,
  owner_id       TEXT NOT NULL,
  status         TEXT NOT NULL CHECK (status IN ('ISSUED', 'CANCELLED', 'REFUNDED')),
  created_at     TEXT NOT NULL,

  FOREIGN KEY (order_id) REFERENCES orders(id),
  FOREIGN KEY (ticket_type_id) REFERENCES ticket_types(id),
  FOREIGN KEY (event_id) REFERENCES events(id),
  FOREIGN KEY (owner_id) REFERENCES users(id)
);

-- Note: Index creation (CREATE INDEX) is not supported by this engine
-- The query planner handles index selection internally
