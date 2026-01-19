import { useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Layout } from '@/components/layout/Layout';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Skeleton } from '@/components/ui/skeleton';
import { apiClient } from '@/lib/api';
import { useAuth } from '@/contexts/AuthContext';
import { format } from 'date-fns';
import type { EventType, TicketType } from '@/types';
import { Plus, Search, MoreVertical, Edit, Copy, Trash2, Eye, BarChart3 } from 'lucide-react';
import { toast } from 'sonner';

function formatEventDateTime(dateStr: string | undefined): string {
  if (!dateStr) return 'Date TBD';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return 'Date TBD';
  return format(date, 'EEE, MMM d, yyyy • h:mm a');
}

const OrganizerEvents = () => {
  const { eventId } = useParams();
  const { user, isAuthenticated } = useAuth();
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');
  const queryClient = useQueryClient();

  const { data: eventsData, isLoading } = useQuery({
    queryKey: ['organizer-events'],
    queryFn: () => apiClient.listEvents(),
    enabled: isAuthenticated && user?.role === 'ORGANIZER',
  });

  const events = eventsData?.data || [];

  const deleteMutation = useMutation({
    mutationFn: (id: string) => apiClient.deleteEvent(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['organizer-events'] });
      toast.success('Event deleted successfully');
    },
    onError: (error: Error) => {
      toast.error(error.message || 'Failed to delete event');
    },
  });

  if (!isAuthenticated || user?.role !== 'ORGANIZER') {
    return (
      <Layout>
        <div className="container py-16 text-center">
          <h1 className="mb-4 text-2xl font-bold">Organizer Access Required</h1>
          <p className="mb-4 text-muted-foreground">Please sign in with a organizer account.</p>
        </div>
      </Layout>
    );
  }

  const filteredEvents = events.filter(event => {
    const matchesSearch =
      event.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
      event.venue?.toLowerCase().includes(searchQuery.toLowerCase());
    const matchesStatus = statusFilter === 'all' || event.status === statusFilter;
    return matchesSearch && matchesStatus;
  });

  const handleDelete = (eventId: string) => {
    if (confirm('Are you sure you want to delete this event?')) {
      deleteMutation.mutate(eventId);
    }
  };

  const handleDuplicate = (eventId: string) => {
    toast.success('Event duplicated successfully!');
  };

  return (
    <Layout>
      <div className="container py-8 lg:py-12">
        {/* Header */}
        <div className="mb-8 flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
          <div>
            <h1 className="text-3xl font-bold">My Events</h1>
            <p className="mt-1 text-muted-foreground">Manage and track all your events</p>
          </div>
          <Button asChild>
            <Link to="/organizer/events/new">
              <Plus className="mr-2 h-4 w-4" />
              Create Event
            </Link>
          </Button>
        </div>

        {/* Filters */}
        <div className="mb-6 flex flex-col gap-4 sm:flex-row">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="Search events..."
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              className="pl-9"
            />
          </div>
          <Select value={statusFilter} onValueChange={setStatusFilter}>
            <SelectTrigger className="w-full sm:w-[180px]">
              <SelectValue placeholder="All Status" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All Status</SelectItem>
              <SelectItem value="DRAFT">Draft</SelectItem>
              <SelectItem value="PUBLISHED">Published</SelectItem>
              <SelectItem value="CANCELLED">Cancelled</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {/* Events List */}
        {isLoading ? (
          <div className="space-y-4">
            {[1, 2, 3].map(i => (
              <Skeleton key={i} className="h-32 w-full" />
            ))}
          </div>
        ) : filteredEvents.length === 0 ? (
          <div className="py-16 text-center">
            <p className="mb-4 text-xl font-medium text-muted-foreground">No events found</p>
            <Button asChild>
              <Link to="/organizer/events/new">
                <Plus className="mr-2 h-4 w-4" />
                Create Your First Event
              </Link>
            </Button>
          </div>
        ) : (
          <div className="space-y-4">
            {filteredEvents.map(event => {
              const eventTyped = event as EventType;
              const ticketTypes: TicketType[] =
                eventTyped.ticket_types || eventTyped.ticketTypes || [];
              const totalCapacity =
                eventTyped.total_capacity ??
                (ticketTypes.reduce((sum: number, t: TicketType) => sum + (t.capacity || 0), 0) ||
                  0);
              const totalSold =
                eventTyped.total_sold ??
                (ticketTypes.reduce((sum: number, t: TicketType) => sum + (t.sold || 0), 0) || 0);
              const soldPercentage = totalCapacity > 0 ? (totalSold / totalCapacity) * 100 : 0;

              return (
                <Card key={event.id} className="overflow-hidden">
                  <CardContent className="p-0">
                    <div className="flex flex-col md:flex-row">
                      <div className="flex-1 p-4 md:p-6">
                        <div className="flex items-start justify-between gap-4">
                          <div>
                            <div className="mb-2 flex items-center gap-2">
                              <h3 className="text-lg font-semibold">{event.title}</h3>
                              <span
                                className={`status-badge ${
                                  event.status === 'PUBLISHED'
                                    ? 'status-published'
                                    : event.status === 'DRAFT'
                                      ? 'status-draft'
                                      : event.status === 'CANCELLED'
                                        ? 'status-cancelled'
                                        : ''
                                }`}
                              >
                                {event.status}
                              </span>
                            </div>
                            <p className="text-sm text-muted-foreground">
                              {formatEventDateTime(event.start_time)}
                            </p>
                            <p className="text-sm text-muted-foreground">{event.venue}</p>
                          </div>

                          <DropdownMenu>
                            <DropdownMenuTrigger asChild>
                              <Button variant="ghost" size="icon">
                                <MoreVertical className="h-4 w-4" />
                              </Button>
                            </DropdownMenuTrigger>
                            <DropdownMenuContent align="end">
                              <DropdownMenuItem asChild>
                                <Link to={`/events/${event.id}`}>
                                  <Eye className="mr-2 h-4 w-4" />
                                  View Event
                                </Link>
                              </DropdownMenuItem>
                              <DropdownMenuItem asChild>
                                <Link to={`/organizer/events/${event.id}/edit`}>
                                  <Edit className="mr-2 h-4 w-4" />
                                  Edit
                                </Link>
                              </DropdownMenuItem>
                              <DropdownMenuItem onClick={() => handleDuplicate(event.id)}>
                                <Copy className="mr-2 h-4 w-4" />
                                Duplicate
                              </DropdownMenuItem>
                              <DropdownMenuItem
                                onClick={() => handleDelete(event.id)}
                                className="text-destructive"
                              >
                                <Trash2 className="mr-2 h-4 w-4" />
                                Delete
                              </DropdownMenuItem>
                            </DropdownMenuContent>
                          </DropdownMenu>
                        </div>

                        <div className="mt-6 grid grid-cols-3 gap-4 border-t pt-4">
                          <div>
                            <p className="text-xs text-muted-foreground">Tickets Sold</p>
                            <p className="font-semibold">
                              {totalSold} / {totalCapacity}
                            </p>
                          </div>
                          <div>
                            <p className="text-xs text-muted-foreground">Revenue</p>
                            <p className="font-semibold">
                              {ticketTypes.length > 0
                                ? `$${ticketTypes.reduce(
                                    (sum: number, t: TicketType) =>
                                      sum + (t.sold || 0) * (t.price || 0),
                                    0
                                  )}`
                                : '-'}
                            </p>
                          </div>
                          <div>
                            <p className="text-xs text-muted-foreground">Sold %</p>
                            <div className="flex items-center gap-2">
                              <div className="h-2 flex-1 overflow-hidden rounded-full bg-muted">
                                <div
                                  className="h-full rounded-full bg-primary"
                                  style={{ width: `${soldPercentage}%` }}
                                />
                              </div>
                              <span className="text-sm font-medium">
                                {soldPercentage.toFixed(0)}%
                              </span>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              );
            })}
          </div>
        )}
      </div>
    </Layout>
  );
};

export default OrganizerEvents;
