import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Layout } from '@/components/layout/Layout';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { apiClient } from '@/lib/api';
import { useAuth } from '@/contexts/AuthContext';
import {
  CalendarDays,
  DollarSign,
  Users,
  Ticket,
  TrendingUp,
  Plus,
  ArrowRight,
  BarChart3,
} from 'lucide-react';

function formatEventDate(dateStr: string | undefined): string {
  if (!dateStr) return 'Date TBD';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return 'Date TBD';
  return date.toLocaleDateString();
}

const OrganizerDashboard = () => {
  const { user, isAuthenticated, login } = useAuth();

  const { data: eventsData, isLoading: eventsLoading } = useQuery({
    queryKey: ['organizer-events'],
    queryFn: () => apiClient.listEvents(),
    enabled: isAuthenticated && user?.role === 'ORGANIZER',
  });

  const { data: ordersData, isLoading: ordersLoading } = useQuery({
    queryKey: ['organizer-orders'],
    queryFn: () => apiClient.listOrders(),
    enabled: isAuthenticated && user?.role === 'ORGANIZER',
  });

  if (!isAuthenticated || user?.role !== 'ORGANIZER') {
    return (
      <Layout>
        <div className="container py-16 text-center">
          <h1 className="mb-4 text-2xl font-bold">Organizer Access Required</h1>
          <p className="mb-8 text-muted-foreground">
            Sign in with a organizer account to access the dashboard.
          </p>
          <Button onClick={login}>Sign In with Google</Button>
        </div>
      </Layout>
    );
  }

  const events = eventsData?.data || [];
  const orders = ordersData || [];
  const publishedEvents = events.filter(e => e.status === 'PUBLISHED');
  const draftEvents = events.filter(e => e.status === 'DRAFT');

  const totalRevenue = orders
    .filter(o => o.status === 'PAID')
    .reduce((sum, o) => sum + o.total_amount, 0);

  const totalTicketsSold = orders.reduce((sum, o) => sum + (o.tickets?.length || 0), 0);

  const stats = [
    {
      title: 'Total Revenue',
      value: `$${totalRevenue.toLocaleString()}`,
      change: '+12.5%',
      icon: DollarSign,
      color: 'text-success',
    },
    {
      title: 'Tickets Sold',
      value: totalTicketsSold.toString(),
      change: '+8.2%',
      icon: Ticket,
      color: 'text-info',
    },
    {
      title: 'Active Events',
      value: publishedEvents.length.toString(),
      change: '+2',
      icon: CalendarDays,
      color: 'text-primary',
    },
    {
      title: 'Total Attendees',
      value: '1,234',
      change: '+15.3%',
      icon: Users,
      color: 'text-accent',
    },
  ];

  const recentOrders = orders.slice(0, 5);

  return (
    <Layout>
      <div className="container py-8 lg:py-12">
        <div className="mb-8 flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
          <div>
            <h1 className="text-3xl font-bold">Dashboard</h1>
            <p className="mt-1 text-muted-foreground">Welcome back, {user?.name || 'Organizer'}</p>
          </div>
          <Button asChild>
            <Link to="/organizer/events/new">
              <Plus className="mr-2 h-4 w-4" />
              Create Event
            </Link>
          </Button>
        </div>

        <div className="mb-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {stats.map(stat => (
            <Card key={stat.title}>
              <CardContent className="p-6">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-sm text-muted-foreground">{stat.title}</p>
                    <p className="mt-1 text-2xl font-bold">{stat.value}</p>
                  </div>
                  <div
                    className={`flex h-12 w-12 items-center justify-center rounded-xl bg-muted ${stat.color}`}
                  >
                    <stat.icon className="h-6 w-6" />
                  </div>
                </div>
                <div className="mt-3 flex items-center gap-1 text-sm">
                  <TrendingUp className="h-4 w-4 text-success" />
                  <span className="font-medium text-success">{stat.change}</span>
                  <span className="text-muted-foreground">vs last month</span>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>

        <div className="grid gap-8 lg:grid-cols-3">
          <div className="lg:col-span-2">
            <Card>
              <CardHeader className="flex flex-row items-center justify-between">
                <CardTitle>Your Events</CardTitle>
                <Button variant="ghost" size="sm" asChild>
                  <Link to="/organizer/events">
                    View All
                    <ArrowRight className="ml-2 h-4 w-4" />
                  </Link>
                </Button>
              </CardHeader>
              <CardContent>
                {eventsLoading ? (
                  <div className="space-y-4">
                    {[1, 2, 3].map(i => (
                      <Skeleton key={i} className="h-16 w-full" />
                    ))}
                  </div>
                ) : events.length > 0 ? (
                  <div className="space-y-4">
                    {events.slice(0, 4).map(event => (
                      <Link
                        key={event.id}
                        to={`/organizer/events/${event.id}/edit`}
                        className="flex items-center gap-4 rounded-lg p-3 transition-colors hover:bg-muted"
                      >
                        <div className="flex h-14 w-14 items-center justify-center rounded-lg bg-muted">
                          <CalendarDays className="h-6 w-6 text-muted-foreground" />
                        </div>
                        <div className="min-w-0 flex-1">
                          <p className="truncate font-medium">{event.title}</p>
                          <p className="text-sm text-muted-foreground">
                            {formatEventDate(event.start_time)}
                          </p>
                        </div>
                        <span
                          className={`status-badge ${
                            event.status === 'PUBLISHED'
                              ? 'status-published'
                              : event.status === 'DRAFT'
                                ? 'status-draft'
                                : 'status-cancelled'
                          }`}
                        >
                          {event.status}
                        </span>
                      </Link>
                    ))}
                  </div>
                ) : (
                  <div className="py-8 text-center">
                    <p className="mb-4 text-muted-foreground">No events yet</p>
                    <Button asChild variant="outline">
                      <Link to="/organizer/events/new">Create Your First Event</Link>
                    </Button>
                  </div>
                )}
              </CardContent>
            </Card>
          </div>

          <div className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>Quick Actions</CardTitle>
              </CardHeader>
              <CardContent className="grid gap-3">
                <Button asChild variant="outline" className="justify-start">
                  <Link to="/organizer/events/new">
                    <Plus className="mr-2 h-4 w-4" />
                    Create New Event
                  </Link>
                </Button>
                <Button asChild variant="outline" className="justify-start">
                  <Link to="/organizer/events">
                    <CalendarDays className="mr-2 h-4 w-4" />
                    Manage Events
                  </Link>
                </Button>
                <Button asChild variant="outline" className="justify-start">
                  <Link to="/organizer/analytics">
                    <BarChart3 className="mr-2 h-4 w-4" />
                    View Analytics
                  </Link>
                </Button>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Recent Sales</CardTitle>
              </CardHeader>
              <CardContent>
                {ordersLoading ? (
                  <div className="space-y-4">
                    {[1, 2, 3].map(i => (
                      <Skeleton key={i} className="h-10 w-full" />
                    ))}
                  </div>
                ) : recentOrders.length > 0 ? (
                  <div className="space-y-4">
                    {recentOrders.map(order => (
                      <div key={order.id} className="flex items-center gap-3">
                        <div className="gradient-primary flex h-10 w-10 items-center justify-center rounded-full text-sm font-medium text-primary-foreground">
                          {order.id.charAt(0).toUpperCase()}
                        </div>
                        <div className="min-w-0 flex-1">
                          <p className="text-sm font-medium">Order #{order.id.slice(-6)}</p>
                          <p className="text-xs text-muted-foreground">
                            {order.tickets?.length || 0} ticket(s)
                          </p>
                        </div>
                        <p className="font-medium">${order.total_amount}</p>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="py-4 text-center text-sm text-muted-foreground">No recent sales</p>
                )}
              </CardContent>
            </Card>
          </div>
        </div>
      </div>
    </Layout>
  );
};

export default OrganizerDashboard;
