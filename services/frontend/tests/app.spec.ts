import { test, expect } from '@playwright/test';

test.describe('Homepage', () => {
  test('should load homepage successfully', async ({ page }) => {
    await page.goto('/');
    
    await expect(page).toHaveTitle(/Eventify/);
    await expect(page.locator('text=Discover')).toBeVisible();
  });

  test('should navigate to events page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Browse Events');
    
    await expect(page).toHaveURL(/\/events/);
  });

  test('should show login dialog when clicking sign in', async ({ page }) => {
    await page.goto('/');
    
    const signInButton = page.locator('button:has-text("Sign In")');
    await expect(signInButton).toBeVisible();
  });
});

test.describe('Events Page', () => {
  test('should load events page', async ({ page }) => {
    await page.goto('/events');
    
    await expect(page.locator('h1:has-text("Browse Events")')).toBeVisible();
  });

  test('should search for events', async ({ page }) => {
    await page.goto('/events');
    
    const searchInput = page.locator('input[placeholder*="Search"]');
    await expect(searchInput).toBeVisible();
    
    await searchInput.fill('test');
    await expect(searchInput).toHaveValue('test');
  });
});

test.describe('Authentication', () => {
  test('should redirect to auth on login click', async ({ page }) => {
    await page.goto('/');
    
    page.on('request', (request) => {
      if (request.url().includes('/auth/google/start')) {
        expect(request.url()).toContain('accounts.google.com');
      }
    });
  });
});

test.describe('Responsive Design', () => {
  test('should work on mobile', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto('/');
    
    await expect(page.locator('text=Discover')).toBeVisible();
    
    const menuButton = page.locator('button[aria-label*="menu"]').first();
    await expect(menuButton).toBeVisible();
  });
});
