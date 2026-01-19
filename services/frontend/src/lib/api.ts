import type {
  User,
  EventType,
  TicketType,
  Order,
  Ticket,
  AuthResponse,
  MeResponse,
  CreateEventRequest,
  UpdateEventRequest,
  CreateTicketTypeRequest,
  UpdateTicketTypeRequest,
  CreateOrderRequest,
  ListEventsQuery,
  PaginatedResponse,
  EventWithTicketTypes,
} from '@/types';

const API_BASE_URL = import.meta.env?.VITE_API_BASE_URL || 'http://localhost:8080';
const MOCK_MODE = import.meta.env?.VITE_USE_MOCK_AUTH === 'true';
const STORAGE_KEY = 'auth_token';

interface MockUser {
  id: number;
  email: string;
  name: string;
  avatarUrl?: string;
  role: 'ORGANIZER' | 'CUSTOMER';
}

const MOCK_USERS: Record<string, MockUser> = {
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

function createMockToken(user: MockUser): string {
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
}

class ApiClient {
  private token: string | null = null;
  private mockUser: MockUser | null = null;

  setToken(token: string | null) {
    this.token = token;
    if (token) {
      sessionStorage.setItem(STORAGE_KEY, token);
      try {
        const payload = this.decodeToken(token);
        if (payload && payload.email) {
          this.mockUser = MOCK_USERS[payload.email] || null;
        }
      } catch {
        this.mockUser = null;
      }
    } else {
      sessionStorage.removeItem(STORAGE_KEY);
      this.mockUser = null;
    }
  }

  getToken(): string | null {
    if (!this.token) {
      this.token = sessionStorage.getItem(STORAGE_KEY);
      if (this.token) {
        try {
          const payload = this.decodeToken(this.token);
          if (payload && payload.email) {
            this.mockUser = MOCK_USERS[payload.email] || null;
          }
        } catch {
          this.mockUser = null;
        }
      }
    }
    return this.token;
  }

  private decodeToken(token: string): Record<string, unknown> | null {
    try {
      const parts = token.split('.');
      if (parts.length !== 3) return null;
      const payload = parts[1].replace(/-/g, '+').replace(/_/g, '/');
      const decoded = atob(payload);
      return JSON.parse(decoded);
    } catch {
      return null;
    }
  }

  isMockMode(): boolean {
    return MOCK_MODE;
  }

  async loginWithGoogle(): Promise<void> {
    if (MOCK_MODE) {
      window.location.href = `${API_BASE_URL}/auth/google/start?mock=true`;
    } else {
      window.location.href = `${API_BASE_URL}/auth/google/start`;
    }
  }

  async mockLogin(email: string): Promise<void> {
    const user = MOCK_USERS[email];
    if (!user) {
      throw new Error(`Mock user not found: ${email}`);
    }
    this.mockUser = user;
    const token = createMockToken(user);
    this.setToken(token);
  }

  async handleAuthCallback(): Promise<AuthResponse> {
    if (MOCK_MODE) {
      const hash = window.location.hash;
      const token = hash.split('token=')[1]?.split('#')[0];
      if (token) {
        this.setToken(token);
        const user = this.getUserFromToken();
        return { token, user: user as User };
      }
      throw new Error('No token in callback');
    }

    const response = await this.request<AuthResponse>('/auth/google/callback');
    this.setToken(response.token);
    return response;
  }

  private getUserFromToken(): Partial<User> | null {
    const token = this.getToken();
    if (!token) return null;
    try {
      const payload = this.decodeToken(token);
      if (!payload) return null;
      return {
        id: parseInt(payload.sub as string) || 0,
        email: payload.email as string,
        name: payload.name as string,
        role: (payload.role as User['role']) || 'CUSTOMER',
        googleSub: payload.sub as string,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    } catch {
      return null;
    }
  }

  async getMe(): Promise<User> {
    if (this.mockUser) {
      return {
        id: this.mockUser.id.toString(),
        googleSub: this.mockUser.id.toString(),
        email: this.mockUser.email,
        name: this.mockUser.name,
        avatarUrl: this.mockUser.avatarUrl,
        role: this.mockUser.role,
        phone: null,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }

    if (MOCK_MODE) {
      const user = this.getUserFromToken();
      if (user) return user as User;
      throw new Error('Not authenticated');
    }

    const response = await this.request<MeResponse>('/v1/users/me');
    return response.user;
  }

  async logout(): Promise<void> {
    this.setToken(null);
  }

  async listEvents(query: ListEventsQuery = {}): Promise<PaginatedResponse<EventType>> {
    if (MOCK_MODE) {
      const now = new Date();
      const futureDate = (days: number) => new Date(now.getTime() + 86400000 * days).toISOString();
      return {
        data: [
          {
            id: 1,
            organizer_user_id: 2,
            title: 'Music Festival 2024',
            description: 'Annual music festival',
            venue: 'Central Park',
            location: 'New York, NY',
            start_time: futureDate(30),
            end_time: futureDate(30) as unknown as string,
            status: 'PUBLISHED',
            created_at: new Date(Date.now() - 86400000 * 7).toISOString(),
            updated_at: new Date().toISOString(),
          },
          {
            id: 2,
            organizer_user_id: 2,
            title: 'Tech Conference 2024',
            description: 'Technology conference',
            venue: 'Convention Center',
            location: 'San Francisco, CA',
            start_time: futureDate(60),
            end_time: futureDate(61),
            status: 'PUBLISHED',
            created_at: new Date(Date.now() - 86400000 * 14).toISOString(),
            updated_at: new Date().toISOString(),
          },
          {
            id: 3,
            organizer_user_id: 2,
            title: 'Art Exhibition Opening',
            description: 'Modern art exhibition',
            venue: 'City Gallery',
            location: 'Los Angeles, CA',
            start_time: futureDate(14),
            end_time: futureDate(14) as unknown as string,
            status: 'PUBLISHED',
            created_at: new Date(Date.now() - 86400000 * 3).toISOString(),
            updated_at: new Date().toISOString(),
          },
        ],
        page: 1,
        limit: 50,
        total: 3,
        totalPages: 1,
      };
    }

    const params = new URLSearchParams();
    if (query.status) params.append('status', query.status);
    if (query.from) params.append('from', query.from);
    if (query.to) params.append('to', query.to);
    if (query.q) params.append('q', query.q);
    if (query.page) params.append('page', query.page.toString());
    if (query.limit) params.append('limit', query.limit.toString());

    const queryString = params.toString();
    const endpoint = `/v1/events${queryString ? `?${queryString}` : ''}`;
    return this.request<PaginatedResponse<EventType>>(endpoint);
  }

  async getEvent(eventId: string): Promise<EventWithTicketTypes> {
    if (MOCK_MODE) {
      const eventNumId = parseInt(eventId);
      return {
        event: {
          id: eventNumId,
          organizer_user_id: 2,
          title: 'Music Festival 2024',
          description:
            'Join us for an unforgettable experience at the annual Music Festival 2024! Featuring world-class artists, delicious food vendors, and an incredible atmosphere.',
          venue: 'Central Park',
          location: 'New York, NY',
          start_time: new Date(Date.now() + 86400000 * 30).toISOString(),
          end_time: new Date(Date.now() + 86400000 * 30 + 43200000).toISOString(),
          status: 'PUBLISHED',
          created_at: new Date(Date.now() - 86400000 * 7).toISOString(),
          updated_at: new Date().toISOString(),
        },
        ticket_types: [
          {
            id: 1,
            event_id: eventNumId,
            name: 'VIP',
            price: 200,
            capacity: 500,
            sold: 234,
            remaining: 266,
            created_at: new Date(Date.now() - 86400000 * 7).toISOString(),
            updated_at: new Date().toISOString(),
          },
          {
            id: 2,
            event_id: eventNumId,
            name: 'General Admission',
            price: 75,
            capacity: 2000,
            sold: 1567,
            remaining: 433,
            created_at: new Date(Date.now() - 86400000 * 7).toISOString(),
            updated_at: new Date().toISOString(),
          },
          {
            id: 3,
            event_id: eventNumId,
            name: 'Early Bird',
            price: 50,
            capacity: 500,
            sold: 500,
            remaining: 0,
            created_at: new Date(Date.now() - 86400000 * 30).toISOString(),
            updated_at: new Date(Date.now() - 86400000 * 7).toISOString(),
          },
        ],
        organizer_name: 'Event Organizer Inc.',
      };
    }
    return this.request<EventWithTicketTypes>(`/v1/events/${eventId}`);
  }

  async createEvent(event: CreateEventRequest): Promise<EventType> {
    if (MOCK_MODE) {
      return {
        id: Math.floor(Math.random() * 10000),
        organizer_user_id: 1,
        title: event.title,
        description: event.description,
        venue: event.venue,
        location: event.location,
        start_time: event.start_time,
        end_time: event.end_time,
        status: 'DRAFT',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    const response = await this.request<{ event: EventType }>('/v1/events', {
      method: 'POST',
      body: JSON.stringify(event),
    });
    return response.event;
  }

  async updateEvent(eventId: string, event: UpdateEventRequest): Promise<EventType> {
    if (MOCK_MODE) {
      return {
        id: parseInt(eventId),
        organizer_user_id: 1,
        title: event.title || 'Updated Event',
        description: event.description,
        venue: event.venue,
        location: event.location,
        start_time: event.start_time || new Date().toISOString(),
        end_time: event.end_time || new Date().toISOString(),
        status: 'DRAFT',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    const response = await this.request<EventType>(`/v1/events/${eventId}`, {
      method: 'PATCH',
      body: JSON.stringify(event),
    });
    return response;
  }

  async publishEvent(eventId: string): Promise<EventType> {
    if (MOCK_MODE) {
      return {
        id: parseInt(eventId),
        organizer_user_id: 1,
        title: 'Published Event',
        description: 'Published event',
        venue: 'Venue',
        location: 'Location',
        start_time: new Date().toISOString(),
        end_time: new Date().toISOString(),
        status: 'PUBLISHED',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    const response = await this.request<EventType>(`/v1/events/${eventId}/publish`, {
      method: 'POST',
    });
    return response;
  }

  async deleteEvent(eventId: string): Promise<void> {
    if (MOCK_MODE) return;
    await this.request(`/v1/events/${eventId}`, { method: 'DELETE' });
  }

  async createTicketType(
    eventId: string,
    ticketType: CreateTicketTypeRequest
  ): Promise<TicketType> {
    if (MOCK_MODE) {
      return {
        id: Math.floor(Math.random() * 10000),
        event_id: parseInt(eventId),
        name: ticketType.name,
        price: ticketType.price,
        capacity: ticketType.capacity,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    const response = await this.request<{ ticket_type: TicketType }>(
      `/v1/events/${eventId}/ticket-types`,
      { method: 'POST', body: JSON.stringify(ticketType) }
    );
    return response.ticket_type;
  }

  async listTicketTypes(eventId: string): Promise<TicketType[]> {
    if (MOCK_MODE) return [];
    const response = await this.request<{ ticket_types: TicketType[] }>(
      `/v1/events/${eventId}/ticket-types`
    );
    return response.ticket_types;
  }

  async updateTicketType(
    eventId: string,
    ticketTypeId: string,
    ticketType: UpdateTicketTypeRequest
  ): Promise<TicketType> {
    if (MOCK_MODE) {
      return {
        id: parseInt(ticketTypeId),
        event_id: parseInt(eventId),
        name: ticketType.name || 'Ticket',
        price: ticketType.price || 0,
        capacity: ticketType.capacity || 100,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    const response = await this.request<{ ticket_type: TicketType }>(
      `/v1/events/${eventId}/ticket-types/${ticketTypeId}`,
      { method: 'PATCH', body: JSON.stringify(ticketType) }
    );
    return response.ticket_type;
  }

  async deleteTicketType(eventId: string, ticketTypeId: string): Promise<void> {
    if (MOCK_MODE) return;
    await this.request<void>(`/v1/events/${eventId}/ticket-types/${ticketTypeId}`, {
      method: 'DELETE',
    });
  }

  async listOrders(): Promise<Order[]> {
    if (MOCK_MODE) {
      return [
        {
          id: '1001',
          customer_user_id: 1,
          total_amount: 210,
          status: 'PAID',
          created_at: new Date(Date.now() - 86400000).toISOString(),
          updated_at: new Date(Date.now() - 86400000).toISOString(),
        },
        {
          id: '1002',
          customer_user_id: 1,
          total_amount: 150,
          status: 'PENDING',
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ];
    }
    const response = await this.request<{ orders: Order[] }>('/v1/orders');
    return response.orders;
  }

  async getOrder(orderId: string): Promise<Order> {
    if (MOCK_MODE) {
      return {
        id: parseInt(orderId),
        customer_user_id: 1,
        total_amount: 210,
        status: 'PAID',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    const response = await this.request<{ order: Order }>(`/v1/orders/${orderId}`);
    return response.order;
  }

  async createOrder(order: CreateOrderRequest): Promise<Order> {
    if (MOCK_MODE) {
      const totalAmount = order.items.reduce((sum, item) => {
        return sum + item.quantity * 100;
      }, 0);
      return {
        id: Math.floor(Math.random() * 10000) + 1000,
        customer_user_id: 1,
        total_amount: totalAmount,
        status: 'PENDING',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    const response = await this.request<{ order: Order }>('/v1/orders', {
      method: 'POST',
      body: JSON.stringify(order),
    });
    return response.order;
  }

  async confirmOrder(orderId: string): Promise<Order> {
    if (MOCK_MODE) {
      return {
        id: parseInt(orderId),
        customer_user_id: 1,
        total_amount: 210,
        status: 'PAID',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    const response = await this.request<{ order: Order }>(`/v1/orders/${orderId}/confirm`, {
      method: 'POST',
    });
    return response.order;
  }

  async listTickets(): Promise<Ticket[]> {
    if (MOCK_MODE) {
      return [
        {
          id: '5001',
          order_id: '1001',
          ticket_type_id: '1',
          unit_price: 100,
          status: 'ISSUED',
          created_at: new Date(Date.now() - 86400000).toISOString(),
          ticket_type_name: 'VIP',
          ticket_type_price: 100,
          event_id: '1',
          event_title: 'Music Festival 2024',
          event_start_time: '2024-06-15T10:00:00',
          event_end_time: '2024-06-15T22:00:00',
          event_venue: 'Central Park',
          event_location: 'New York, NY',
        },
        {
          id: '5002',
          order_id: '1001',
          ticket_type_id: '2',
          unit_price: 55,
          status: 'ISSUED',
          created_at: new Date(Date.now() - 86400000).toISOString(),
          ticket_type_name: 'General Admission',
          ticket_type_price: 55,
          event_id: '1',
          event_title: 'Music Festival 2024',
          event_start_time: '2024-06-15T10:00:00',
          event_end_time: '2024-06-15T22:00:00',
          event_venue: 'Central Park',
          event_location: 'New York, NY',
        },
      ];
    }
    const response = await this.request<{ tickets: Ticket[] }>('/v1/tickets');
    return response.tickets;
  }

  async updateUserRole(userId: string, role: 'ORGANIZER' | 'CUSTOMER'): Promise<void> {
    if (MOCK_MODE) return;
    await this.request(`/v1/admin/users/${userId}/role`, {
      method: 'POST',
      body: JSON.stringify({ user_id: parseInt(userId), role }),
    });
  }

  private async request<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
    const url = `${API_BASE_URL}${endpoint}`;
    const token = this.getToken();

    const headers: HeadersInit = {
      'Content-Type': 'application/json',
      ...(options.headers || {}),
    };

    if (token) {
      (headers as Record<string, string>)['Authorization'] = `Bearer ${token}`;
    }

    try {
      const response = await fetch(url, { ...options, headers });
      if (!response.ok) {
        const contentType = response.headers.get('content-type');
        if (contentType && contentType.includes('application/json')) {
          const error = await response.json().catch(() => ({}));
          throw new Error(error.message || `HTTP error! status: ${response.status}`);
        }
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      if (response.status === 204) {
        return undefined as T;
      }
      const contentType = response.headers.get('content-type');
      if (contentType && contentType.includes('application/json')) {
        return response.json();
      }
      return undefined as T;
    } catch (error) {
      if (error instanceof Error) throw error;
      throw new Error('An unexpected error occurred');
    }
  }
}

export const apiClient = new ApiClient();
