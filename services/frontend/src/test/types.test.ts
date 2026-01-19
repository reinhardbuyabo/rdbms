import { describe, it, expect } from 'vitest';
import * as api from '../lib/api';
import * as utils from '../lib/utils';

describe('API Client', () => {
  it('should have all required methods', () => {
    expect(api.apiClient.setToken).toBeDefined();
    expect(api.apiClient.getToken).toBeDefined();
    expect(api.apiClient.loginWithGoogle).toBeDefined();
    expect(api.apiClient.handleAuthCallback).toBeDefined();
    expect(api.apiClient.getMe).toBeDefined();
    expect(api.apiClient.logout).toBeDefined();
    expect(api.apiClient.listEvents).toBeDefined();
    expect(api.apiClient.getEvent).toBeDefined();
    expect(api.apiClient.createEvent).toBeDefined();
    expect(api.apiClient.updateEvent).toBeDefined();
    expect(api.apiClient.deleteEvent).toBeDefined();
    expect(api.apiClient.listOrders).toBeDefined();
    expect(api.apiClient.listTickets).toBeDefined();
  });

  it('should have correct API endpoint methods', () => {
    expect(typeof api.apiClient.loginWithGoogle).toBe('function');
    expect(typeof api.apiClient.listEvents).toBe('function');
    expect(typeof api.apiClient.getEvent).toBe('function');
    expect(typeof api.apiClient.createEvent).toBe('function');
    expect(typeof api.apiClient.updateEvent).toBe('function');
    expect(typeof api.apiClient.deleteEvent).toBe('function');
  });
});

describe('Utilities', () => {
  it('should have cn utility', () => {
    expect(utils.cn).toBeDefined();
    expect(typeof utils.cn).toBe('function');

    expect(utils.cn('class1', 'class2')).toBe('class1 class2');
    expect(utils.cn('class1', { class2: true })).toBe('class1 class2');
    expect(utils.cn('class1', { class2: false })).toBe('class1');
  });

  it('cn should handle multiple inputs', () => {
    expect(utils.cn('a', 'b', 'c')).toBe('a b c');
    expect(utils.cn('a', null, 'b')).toBe('a b');
    expect(utils.cn('a', undefined, 'b')).toBe('a b');
  });
});

describe('Types Module', () => {
  it('should export types file', async () => {
    const types = await import('../types/index.ts');
    expect(types).toBeDefined();
  });
});
