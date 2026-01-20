# User Journey Testing Guide

## Overview

This document provides comprehensive testing instructions for the Eventify Event Ticketing Platform. It covers both ORGANIZER and CUSTOMER user journeys, ensuring all pathways work correctly before testing with real Google OAuth authentication.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Mock Authentication Setup](#mock-authentication-setup)
3. [Organizer User Journey](#organizer-user-journey)
4. [Customer User Journey](#customer-user-journey)
5. [Running Tests](#running-tests)
6. [Troubleshooting](#troubleshooting)

---

## Quick Start

### Prerequisites

```bash
# Frontend
cd /home/reinhard/jan-capstone/services/frontend
npm install

# Backend
cd /home/reinhard/jan-capstone
cargo build --release
```

### Start with Mock Mode

```bash
# Terminal 1 - Backend with mock mode
cd /home/reinhard/jan-capstone
MOCK_MODE=true FRONTEND_URL=http://localhost:5173 JWT_SECRET=test-secret cargo run -p backend_service

# Terminal 2 - Frontend with mock auth
cd /home/reinhard/jan-capstone/services/frontend
VITE_USE_MOCK_AUTH=true npm run dev
```

---

## Mock Authentication Setup

### Mock Users

Two mock users are available for testing:

| Email | Role | Permissions |
|-------|------|-------------|
| `test@example.com` | CUSTOMER | Browse events, purchase tickets, view my tickets |
| `organizer@example.com` | ORGANIZER | Create/manage events, manage ticket types |

### Testing Login

The app will automatically use mock authentication when `VITE_USE_MOCK_AUTH=true` is set. The mock login flow:

1. Click "Sign in with Google"
2. Backend redirects to `/auth-callback?token=<mock-jwt>`
3. App parses token and sets user state
4. User is logged in with role from token

### Switching Users

To switch between user types during testing:

1. Click user avatar/profile menu
2. Click "Sign out"
3. Log in with different mock account

---

## Organizer User Journey

### 1. Authentication

**Expected Flow:**
```
1. Navigate to homepage
2. Click "Sign in with Google"
3. Redirected to auth callback with token
4. User dashboard shows organizer role
```

**Verification:**
- [ ] User is logged in
- [ ] Role is "ORGANIZER"
- [ ] Can access `/organizer/dashboard`
- [ ] Can access `/organizer/events`

### 2. Create New Event

**Test Data:**
```json
{
  "title": "Tech Conference 2024",
  "description": "Annual technology conference featuring industry leaders",
  "venue": "Convention Center",
  "location": "San Francisco, CA",
  "startDate": "2024-09-15",
  "startTime": "09:00",
  "endDate": "2024-09-15",
  "endTime": "18:00"
}
```

**Steps:**
1. Click "Create Event" on organizer dashboard
2. Fill in event details (title, description, venue, dates)
3. Add ticket types:
   - VIP: $200, capacity 500
   - General: $75, capacity 2000
4. Click "Publish Event"

**Verification:**
- [ ] Event is created with status "PUBLISHED"
- [ ] Event appears in organizer events list
- [ ] Event is visible on public events page
- [ ] Ticket types are created with correct pricing/capacity

### 3. Edit Event

**Steps:**
1. Navigate to `/organizer/events`
2. Click edit icon on event card
3. Modify event details
4. Save changes

**Verification:**
- [ ] Changes are persisted
- [ ] Event status remains correct
- [ ] Ticket types can be updated
- [ ] Capacity changes reflect sold tickets

### 4. Manage Ticket Types

**Test Scenarios:**

| Action | Expected Result |
|--------|----------------|
| Create new ticket type | Added to event with correct capacity/price |
| Update ticket price | Price changed, existing orders unaffected |
| Increase capacity | New capacity available |
| Decrease capacity (below sold) | Error: "Cannot reduce below sold" |
| Delete ticket type (no sales) | Successfully deleted |
| Delete ticket type (has sales) | Error: "Cannot delete with sales" |

### 5. Delete Event

**Steps:**
1. Navigate to `/organizer/events`
2. Click delete icon on event card
3. Confirm deletion in dialog

**Verification:**
- [ ] Event is removed from list
- [ ] Associated ticket types are removed
- [ ] Cannot delete event with ticket sales

### 6. View Event Analytics

**Steps:**
1. Navigate to `/organizer/events`
2. View event cards with metrics

**Metrics to Verify:**
- [ ] Tickets sold count
- [ ] Revenue calculation
- [ ] Sold percentage
- [ ] Remaining capacity

---

## Customer User Journey

### 1. Authentication

**Expected Flow:**
```
1. Navigate to homepage
2. Click "Sign in with Google"
3. Redirected to auth callback with token
4. User can access customer features
```

**Verification:**
- [ ] User is logged in
- [ ] Role is "CUSTOMER"
- [ ] Can access browse events
- [ ] Can access cart and checkout

### 2. Browse Events

**Test Scenarios:**

| Filter | Test Data |
|--------|-----------|
| Search query | "Music Festival" |
| Date from | "2024-06-01" |
| Date to | "2024-06-30" |
| Status | PUBLISHED only |

**Verification:**
- [ ] Events list loads
- [ ] Search filters results
- [ ] Date filters work
- [ ] Only published events shown
- [ ] Event cards display correctly

### 3. View Event Details

**Steps:**
1. Click event card on events page
2. View event details
3. Select ticket quantities

**Verification:**
- [ ] Event details display correctly
- [ ] Venue, date, time shown
- [ ] Description visible
- [ ] Ticket types with prices shown
- [ ] Available tickets count correct
- [ ] Sold out tickets disabled

### 4. Add to Cart

**Test Cases:**

| Scenario | Expected Result |
|----------|----------------|
| Add 1 VIP ticket | Cart has 1 item |
| Add multiple quantities | Quantity updates |
| Add different ticket types | Multiple items in cart |
| Add beyond availability | Button disabled at limit |
| Add beyond max (10) | Button disabled at 10 |

**Verification:**
- [ ] Cart count updates
- [ ] Total amount calculates correctly
- [ ] Service fee added (5%)
- [ ] Can update quantities in cart
- [ ] Can remove items from cart

### 5. Checkout Flow

**Test Data:**
```json
{
  "firstName": "John",
  "lastName": "Doe",
  "email": "john@example.com",
  "cardNumber": "4242 4242 4242 4242",
  "expiry": "12/25",
  "cvc": "123"
}
```

**Steps:**
1. Click "Proceed to Checkout" from cart
2. Fill payment form
3. Click "Pay"
4. Wait for processing

**Mock Payment Processing:**
- Payment is simulated (2 second delay)
- Order created with PENDING status
- Order confirmed, status changes to PAID
- Tickets issued

**Verification:**
- [ ] Order created successfully
- [ ] Payment processed
- [ ] Cart cleared
- [ ] Redirected to my-tickets
- [ ] Tickets visible with ISSUED status

### 6. View My Tickets

**Verification:**
- [ ] Purchased tickets display
- [ ] QR code button available
- [ ] Download button available
- [ ] Event link works
- [ ] Tickets categorized as upcoming/past

---

## Running Tests

### Unit Tests

```bash
# Run all frontend tests
cd /home/reinhard/jan-capstone/services/frontend
npm test

# Run specific test file
npm test -- src/test/user-journey.test.tsx

# Run with coverage
npm test -- --coverage
```

### Integration Tests

```bash
# Backend integration tests
cd /home/reinhard/jan-capstone
cargo test -p backend_service

# Full integration test
./scripts/run-integration-tests.sh
```

### E2E Tests (Playwright)

```bash
# Install Playwright
cd /home/reinhard/jan-capstone/services/frontend
npx playwright install

# Run E2E tests
npm run test:e2e

# Run specific E2E test
npm run test:e2e -- --project=chromium --grep="Organizer"
```

---

## Troubleshooting

### Common Issues

#### 1. Mock Login Not Working

**Symptoms:**
- User stays logged out after clicking sign in
- No token in session storage

**Solutions:**
```bash
# Check environment variable
echo $VITE_USE_MOCK_AUTH

# Restart dev server with mock mode
VITE_USE_MOCK_AUTH=true npm run dev
```

#### 2. Events Not Loading

**Symptoms:**
- Empty events list
- Loading spinner persists

**Solutions:**
- Check browser console for API errors
- Verify backend is running
- Check CORS settings

#### 3. Cart Not Persisting

**Symptoms:**
- Cart empties on page refresh

**Solutions:**
- Cart uses local state (not persisted)
- This is expected behavior in mock mode
- Real implementation will persist to backend

#### 4. Token Not Valid

**Symptoms:**
- 401 errors on API calls
- User logged out unexpectedly

**Solutions:**
```bash
# Check JWT secret matches
# Backend: JWT_SECRET=test-secret
# Frontend: Uses token from backend

# Clear session and re-login
sessionStorage.clear()
localStorage.clear()
```

### Debug Mode

Enable debug logging:

```bash
# Backend
RUST_LOG=debug cargo run -p backend_service

# Frontend
VITE_DEBUG=true npm run dev
```

---

## Google OAuth Testing Checklist

Before enabling real Google OAuth, verify:

### Backend
- [ ] `GOOGLE_CLIENT_ID` configured
- [ ] `GOOGLE_CLIENT_SECRET` configured
- [ ] `GOOGLE_REDIRECT_URI` configured
- [ ] JWT_SECRET set
- [ ] FRONTEND_URL matches actual frontend URL

### Frontend
- [ ] `VITE_API_BASE_URL` points to backend
- [ ] `VITE_USE_MOCK_AUTH=false` for production
- [ ] Auth callback route works

### Google Cloud Console
- [ ] OAuth consent screen configured
- [ ] Authorized JavaScript origins:
  - `http://localhost:5173` (development)
  - `https://yourdomain.com` (production)
- [ ] Authorized redirect URIs:
  - `http://localhost:8080/auth/google/callback`
  - `https://yourdomain.com/auth/google/callback`

### Production Readiness
- [ ] Error handling for network failures
- [ ] Token refresh mechanism
- [ ] Logout clears all auth state
- [ ] Role-based access control enforced
- [ ] SQL injection prevention verified
- [ ] XSS protection in place

---

## Test Data Reference

### Mock Events

| ID | Title | Status | Tickets |
|----|-------|--------|---------|
| 1 | Music Festival 2024 | PUBLISHED | VIP (234/500), General (1567/2000), Early Bird (500/500 - SOLD OUT) |
| 2 | Tech Conference 2024 | PUBLISHED | TBA |
| 3 | Art Exhibition Opening | PUBLISHED | TBA |

### Mock Orders

| ID | Customer | Amount | Status |
|----|----------|--------|--------|
| 1001 | test@example.com | $210 | PAID |
| 1002 | test@example.com | $150 | PENDING |

### Mock Tickets

| ID | Order | Event | Type | Status |
|----|-------|-------|------|--------|
| 5001 | 1001 | Music Festival 2024 | VIP | ISSUED |
| 5002 | 1001 | Music Festival 2024 | General | ISSUED |

---

## Next Steps for Production

1. **Security Audit**
   - Review all API endpoints
   - Verify input validation
   - Check rate limiting

2. **Performance**
   - Load test with concurrent users
   - Optimize database queries
   - Implement caching

3. **Monitoring**
   - Set up error tracking (Sentry)
   - Configure logging
   - Create dashboards

4. **Documentation**
   - API documentation
   - User guide
   - Deployment guide
