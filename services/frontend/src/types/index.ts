export type UserRole = 'ORGANIZER' | 'CUSTOMER';

export type EventStatus = 'DRAFT' | 'PUBLISHED' | 'CANCELLED';

export type OrderStatus = 'PENDING' | 'PAID' | 'CANCELLED';

export type TicketStatus = 'HELD' | 'ISSUED' | 'VOID';

export interface User {
  id: string;
  googleSub: string;
  email: string;
  name?: string;
  avatarUrl?: string;
  role: UserRole;
  phone?: string;
  createdAt: string;
  updatedAt: string;
}

export interface EventType {
  id: string;
  organizer_user_id: string;
  title: string;
  description?: string;
  venue?: string;
  location?: string;
  start_time: string;
  end_time: string;
  status: EventStatus;
  created_at: string;
  updated_at: string;
  ticketTypes?: TicketType[];
  ticket_types?: TicketType[];
  total_capacity?: number;
  total_sold?: number;
}

export interface TicketType {
  id: string;
  event_id: string;
  name: string;
  price: number;
  capacity: number;
  sold?: number;
  remaining?: number;
  sales_start?: string;
  sales_end?: string;
  created_at: string;
  updated_at: string;
}

export interface Order {
  id: string;
  customer_user_id: string;
  total_amount: number;
  status: OrderStatus;
  created_at: string;
  updated_at: string;
  tickets?: Ticket[];
}

export interface Ticket {
  id: string;
  order_id: string;
  ticket_type_id: string;
  unit_price: number;
  status: TicketStatus;
  created_at: string;
  ticket_type_name?: string;
  ticket_type_price?: number;
  event_id?: string;
  event_title?: string;
  event_start_time?: string;
  event_end_time?: string;
  event_venue?: string;
  event_location?: string;
}

export interface CartItem {
  ticketType: TicketType;
  quantity: number;
  event: EventType;
}

export interface CreateEventRequest {
  title: string;
  description?: string;
  venue?: string;
  location?: string;
  start_time: string;
  end_time: string;
}

export interface UpdateEventRequest {
  title?: string;
  description?: string;
  venue?: string;
  location?: string;
  start_time?: string;
  end_time?: string;
}

export interface CreateTicketTypeRequest {
  name: string;
  price: number;
  capacity: number;
  salesStart?: string;
  salesEnd?: string;
}

export interface UpdateTicketTypeRequest {
  name?: string;
  price?: number;
  capacity?: number;
  salesStart?: string;
  salesEnd?: string;
}

export interface OrderItemRequest {
  ticketTypeId: string;
  quantity: number;
}

export interface CreateOrderRequest {
  items: OrderItemRequest[];
}

export interface ListEventsQuery {
  status?: EventStatus;
  from?: string;
  to?: string;
  q?: string;
  page?: number;
  limit?: number;
}

export interface PaginatedResponse<T> {
  data: T[];
  page: number;
  limit: number;
  total: number;
  totalPages: number;
}

export interface AuthResponse {
  token: string;
  user: User;
}

export interface MeResponse {
  user: User;
}

export interface ErrorResponse {
  error: string;
  message: string;
}

export interface EventWithTicketTypes {
  event: EventType;
  ticket_types: TicketType[];
  organizer_name?: string;
}
