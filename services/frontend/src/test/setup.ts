import { vi, beforeEach, afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';
import '@testing-library/jest-dom';

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual('react-router-dom');
  return {
    ...actual,
    useNavigate: () => vi.fn(),
    useLocation: () => ({ pathname: '/', search: '', hash: '' }),
    useParams: () => ({}),
  };
});

vi.mock('@/lib/api', () => {
  interface MockUserData {
    id: number;
    email: string;
    name: string;
    role: 'CUSTOMER' | 'ORGANIZER';
    avatarUrl?: string;
  }

  let mockToken: string | null = null;
  let mockUser: MockUserData = {
    id: 1,
    email: 'test@example.com',
    name: 'Test User',
    role: 'CUSTOMER',
  };

  const MOCK_USERS: Record<string, MockUserData> = {
    'test@example.com': {
      id: 1,
      email: 'test@example.com',
      name: 'Test User',
      role: 'CUSTOMER',
    },
    'organizer@example.com': {
      id: 2,
      email: 'organizer@example.com',
      name: 'Organizer User',
      role: 'ORGANIZER',
    },
  };

  const createMockToken = (user: MockUserData): string => {
    const header = btoa(JSON.stringify({ alg: 'HS256', typ: 'JWT' }));
    const payload = btoa(
      JSON.stringify({
        sub: user.id.toString(),
        email: user.email,
        name: user.name,
        role: user.role,
        exp: Date.now() + 86400000,
        iat: Date.now(),
      })
    );
    const signature = btoa('mock-signature');
    return `${header}.${payload}.${signature}`;
  };

  return {
    apiClient: {
      getToken: () => mockToken,
      setToken: (token: string | null) => {
        mockToken = token;
        if (token) {
          try {
            const payload = JSON.parse(
              atob(token.split('.')[1].replace(/-/g, '+').replace(/_/g, '/'))
            );
            if (payload && payload.email) {
              mockUser = MOCK_USERS[payload.email] || null;
            }
          } catch {
            mockUser = null;
          }
        } else {
          mockUser = null;
        }
      },
      loginWithGoogle: vi.fn(),
      mockLogin: vi.fn().mockImplementation(async (email: string) => {
        const user = MOCK_USERS[email];
        if (!user) {
          throw new Error(`Mock user not found: ${email}`);
        }
        mockUser = user;
        const token = createMockToken(user);
        mockToken = token;
      }),
      handleAuthCallback: vi.fn(),
      getMe: vi.fn().mockImplementation(() => {
        if (mockUser) {
          return Promise.resolve({
            id: mockUser.id.toString(),
            googleSub: mockUser.id.toString(),
            email: mockUser.email,
            name: mockUser.name,
            avatarUrl: mockUser.avatarUrl,
            role: mockUser.role,
            phone: null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          });
        }
        return Promise.reject(new Error('Not authenticated'));
      }),
      logout: vi.fn().mockImplementation(() => {
        mockToken = null;
        mockUser = null;
      }),
      listEvents: vi.fn().mockResolvedValue({ data: [] }),
      getEvent: vi.fn(),
      createEvent: vi.fn(),
      updateEvent: vi.fn(),
      deleteEvent: vi.fn(),
      publishEvent: vi.fn(),
      createTicketType: vi.fn(),
      updateTicketType: vi.fn().mockRejectedValue(new Error('Capacity too low')),
      deleteTicketType: vi.fn(),
      listOrders: vi.fn().mockResolvedValue([]),
      getOrder: vi.fn(),
      createOrder: vi.fn().mockResolvedValue({
        id: '1001',
        customer_user_id: '1',
        total_amount: 210,
        status: 'PENDING',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      }),
      confirmOrder: vi.fn().mockResolvedValue({
        id: '1001',
        customer_user_id: '1',
        total_amount: 210,
        status: 'PAID',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      }),
      listTickets: vi.fn().mockResolvedValue([]),
      isMockMode: () => true,
    },
  };
});

beforeEach(() => {
  vi.clearAllMocks();
  cleanup();
});

afterEach(() => {
  vi.restoreAllMocks();
});

Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((_query: string) => ({
    matches: false,
    media: _query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

Object.defineProperty(window, 'navigator', {
  writable: true,
  value: {
    ...window.navigator,
    share: vi.fn(),
    clipboard: {
      writeText: vi.fn(),
    },
  },
});

Object.defineProperty(window, 'location', {
  writable: true,
  value: {
    ...window.location,
    href: 'http://localhost:3000',
    pathname: '/',
    search: '',
    hash: '',
  },
});

global.ResizeObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn(),
}));
