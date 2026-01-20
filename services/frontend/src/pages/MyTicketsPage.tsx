import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Layout } from '@/components/layout/Layout';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Skeleton } from '@/components/ui/skeleton';
import { apiClient } from '@/lib/api';
import { useAuth } from '@/contexts/AuthContext';
import { format } from 'date-fns';
import {
  Ticket,
  QrCode,
  CalendarDays,
  MapPin,
  Download,
  ArrowRight,
  CheckCircle,
} from 'lucide-react';

const MyTicketsPage = () => {
  const { user, isAuthenticated, login, isLoading: authLoading } = useAuth();

  const { data: tickets, isLoading: ticketsLoading } = useQuery({
    queryKey: ['my-tickets'],
    queryFn: () => apiClient.listTickets(),
    enabled: isAuthenticated,
  });

  if (authLoading) {
    return (
      <Layout>
        <div className="container py-8 lg:py-12">
          <Skeleton className="mb-8 h-10 w-48" />
          <div className="grid gap-4">
            {[1, 2, 3].map(i => (
              <Skeleton key={i} className="h-32 w-full" />
            ))}
          </div>
        </div>
      </Layout>
    );
  }

  if (!isAuthenticated) {
    return (
      <Layout>
        <div className="container py-16 text-center">
          <div className="mx-auto max-w-md">
            <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-muted">
              <Ticket className="h-10 w-10 text-muted-foreground" />
            </div>
            <h1 className="mb-2 text-2xl font-bold">Sign in to view your tickets</h1>
            <p className="mb-8 text-muted-foreground">
              Access your purchased tickets and manage your upcoming events.
            </p>
            <Button onClick={login} size="lg">
              Sign In with Google
            </Button>
          </div>
        </div>
      </Layout>
    );
  }

  const allTickets = tickets || [];
  const upcomingTickets = allTickets.filter(t => t.status === 'ISSUED');
  const pastTickets = allTickets.filter(t => t.status !== 'ISSUED');

  const TicketCard = ({ ticket }: { ticket: (typeof allTickets)[0] }) => (
    <Card className="card-hover overflow-hidden">
      <CardContent className="p-0">
        <div className="flex flex-col sm:flex-row">
          <div className="flex-1 p-4 sm:p-6">
            <div className="flex items-start justify-between gap-4">
              <div>
                <Link
                  to={`/events/${ticket.event_id}`}
                  className="text-lg font-semibold transition-colors hover:text-primary"
                >
                  {ticket.event_title || 'Event'}
                </Link>
                <div className="mt-1 flex items-center gap-2">
                  <Badge variant="secondary">{ticket.ticket_type_name || 'Standard'}</Badge>
                  {ticket.status === 'ISSUED' ? (
                    <Badge variant="outline" className="border-success text-success">
                      <CheckCircle className="mr-1 h-3 w-3" />
                      Active
                    </Badge>
                  ) : (
                    <Badge variant="outline">Used</Badge>
                  )}
                </div>
              </div>
            </div>

            <div className="mt-4 grid gap-3 text-sm sm:grid-cols-2">
              <div className="flex items-center gap-2 text-muted-foreground">
                <CalendarDays className="h-4 w-4" />
                {ticket.event_start_time
                  ? format(new Date(ticket.event_start_time), 'EEE, MMM d, yyyy')
                  : 'TBA'}
              </div>
              <div className="flex items-center gap-2 text-muted-foreground">
                <MapPin className="h-4 w-4" />
                <span className="truncate">{ticket.event_venue || 'TBA'}</span>
              </div>
            </div>

            <div className="mt-4 flex items-center gap-3 border-t pt-4">
              <div className="flex-1">
                <p className="text-xs text-muted-foreground">Ticket ID</p>
                <p className="font-mono text-sm">{ticket.id}</p>
              </div>
              <div className="flex gap-2">
                <Button variant="outline" size="sm">
                  <QrCode className="mr-2 h-4 w-4" />
                  View QR
                </Button>
                <Button variant="outline" size="sm">
                  <Download className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );

  return (
    <Layout>
      <div className="container py-8 lg:py-12">
        <div className="mb-8">
          <h1 className="mb-2 text-3xl font-bold">My Tickets</h1>
          <p className="text-muted-foreground">Manage your event tickets and view past events</p>
        </div>

        {ticketsLoading ? (
          <div className="grid gap-4">
            {[1, 2, 3].map(i => (
              <Skeleton key={i} className="h-32 w-full" />
            ))}
          </div>
        ) : allTickets.length === 0 ? (
          <div className="py-16 text-center">
            <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-muted">
              <Ticket className="h-10 w-10 text-muted-foreground" />
            </div>
            <h2 className="mb-2 text-xl font-bold">No tickets yet</h2>
            <p className="mx-auto mb-8 max-w-md text-muted-foreground">
              When you purchase tickets to events, they'll appear here.
            </p>
            <Button asChild size="lg">
              <Link to="/events">
                Browse Events
                <ArrowRight className="ml-2 h-5 w-5" />
              </Link>
            </Button>
          </div>
        ) : (
          <Tabs defaultValue="upcoming" className="space-y-6">
            <TabsList>
              <TabsTrigger value="upcoming">Upcoming ({upcomingTickets.length})</TabsTrigger>
              <TabsTrigger value="past">Past ({pastTickets.length})</TabsTrigger>
            </TabsList>

            <TabsContent value="upcoming" className="space-y-4">
              {upcomingTickets.length === 0 ? (
                <div className="py-12 text-center">
                  <p className="text-muted-foreground">No upcoming events</p>
                  <Button asChild variant="link" className="mt-2">
                    <Link to="/events">Browse events</Link>
                  </Button>
                </div>
              ) : (
                upcomingTickets.map(ticket => <TicketCard key={ticket.id} ticket={ticket} />)
              )}
            </TabsContent>

            <TabsContent value="past" className="space-y-4">
              {pastTickets.length === 0 ? (
                <div className="py-12 text-center">
                  <p className="text-muted-foreground">No past events</p>
                </div>
              ) : (
                pastTickets.map(ticket => <TicketCard key={ticket.id} ticket={ticket} />)
              )}
            </TabsContent>
          </Tabs>
        )}
      </div>
    </Layout>
  );
};

export default MyTicketsPage;
