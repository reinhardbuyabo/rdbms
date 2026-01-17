-- db/seed.sql
-- Seed data for event ticketing system
-- Uses stable IDs for test reproducibility
-- Idempotent: deletes existing seed data before inserting

-- Clean up existing seed data (respect FK order: tickets -> orders -> ticket_types -> events -> users)
DELETE FROM tickets WHERE id IN ('tix_1', 'tix_2');
DELETE FROM orders WHERE id IN ('ord_1', 'ord_2', 'ord_3');
DELETE FROM ticket_types WHERE id IN ('tt_1', 'tt_2', 'tt_3');
DELETE FROM events WHERE id IN ('evt_1', 'evt_2');
DELETE FROM users WHERE id IN ('usr_org_1', 'usr_cus_1', 'usr_cus_2');

-- Insert users
INSERT INTO users (id, role, email, name, avatar_url, created_at) VALUES
  ('usr_org_1', 'ORGANIZER', 'organizer1@example.com', 'Organizer One', NULL, '2026-01-17T00:00:00Z');

INSERT INTO users (id, role, email, name, avatar_url, created_at) VALUES
  ('usr_cus_1', 'CUSTOMER', 'customer1@example.com', 'Customer One', NULL, '2026-01-17T00:00:00Z');

INSERT INTO users (id, role, email, name, avatar_url, created_at) VALUES
  ('usr_cus_2', 'CUSTOMER', 'customer2@example.com', 'Customer Two', NULL, '2026-01-17T00:00:00Z');

-- Insert events
INSERT INTO events (id, organizer_id, title, description, location, starts_at, ends_at, published, created_at) VALUES
  ('evt_1', 'usr_org_1', 'Nairobi Dev Summit', 'Community dev conference', 'Nairobi', '2026-02-01T08:00:00Z', '2026-02-01T17:00:00Z', 1, '2026-01-17T00:00:00Z');

INSERT INTO events (id, organizer_id, title, description, location, starts_at, ends_at, published, created_at) VALUES
  ('evt_2', 'usr_org_1', 'Rust & Databases Meetup', 'Hands-on meetup', 'Nairobi', '2026-02-10T16:00:00Z', '2026-02-10T19:00:00Z', 1, '2026-01-17T00:00:00Z');

-- Insert ticket types
INSERT INTO ticket_types (id, event_id, name, price_cents, capacity, created_at) VALUES
  ('tt_1', 'evt_1', 'General Admission', 150000, 500, '2026-01-17T00:00:00Z');

INSERT INTO ticket_types (id, event_id, name, price_cents, capacity, created_at) VALUES
  ('tt_2', 'evt_1', 'VIP', 350000, 50, '2026-01-17T00:00:00Z');

INSERT INTO ticket_types (id, event_id, name, price_cents, capacity, created_at) VALUES
  ('tt_3', 'evt_2', 'Standard', 50000, 100, '2026-01-17T00:00:00Z');

-- Insert orders
INSERT INTO orders (id, customer_id, event_id, status, total_cents, created_at) VALUES
  ('ord_1', 'usr_cus_1', 'evt_1', 'PAID', 150000, '2026-01-17T00:10:00Z');

INSERT INTO orders (id, customer_id, event_id, status, total_cents, created_at) VALUES
  ('ord_2', 'usr_cus_2', 'evt_1', 'PAID', 350000, '2026-01-17T00:12:00Z');

INSERT INTO orders (id, customer_id, event_id, status, total_cents, created_at) VALUES
  ('ord_3', 'usr_cus_1', 'evt_2', 'PENDING', 50000, '2026-01-17T00:15:00Z');

-- Insert tickets
INSERT INTO tickets (id, order_id, ticket_type_id, event_id, owner_id, status, created_at) VALUES
  ('tix_1', 'ord_1', 'tt_1', 'evt_1', 'usr_cus_1', 'ISSUED', '2026-01-17T00:10:05Z');

INSERT INTO tickets (id, order_id, ticket_type_id, event_id, owner_id, status, created_at) VALUES
  ('tix_2', 'ord_2', 'tt_2', 'evt_1', 'usr_cus_2', 'ISSUED', '2026-01-17T00:12:05Z');
