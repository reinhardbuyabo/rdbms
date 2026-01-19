import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Layout } from '@/components/layout/Layout';
import { Button } from '@/components/ui/button';
import { EventCard } from '@/components/events/EventCard';
import { apiClient } from '@/lib/api';
import { useAuth } from '@/contexts/AuthContext';
import { CalendarDays, Users, Sparkles, ArrowRight, CheckCircle } from 'lucide-react';
import { Skeleton } from '@/components/ui/skeleton';

const Index = () => {
  const { login, isAuthenticated, user } = useAuth();

  const { data: eventsData, isLoading } = useQuery({
    queryKey: ['featured-events'],
    queryFn: () => apiClient.listEvents({ status: 'PUBLISHED', limit: 6 }),
  });

  const events = eventsData?.data || [];
  const featuredEvents = events.slice(0, 3);
  const upcomingEvents = events.slice(0, 6);

  const stats = [
    { value: '10K+', label: 'Events Hosted' },
    { value: '500K+', label: 'Tickets Sold' },
    { value: '50K+', label: 'Happy Attendees' },
    { value: '1K+', label: 'Organizers' },
  ];

  const features = [
    {
      icon: CalendarDays,
      title: 'Easy Event Management',
      description:
        'Create and manage events with our intuitive dashboard. Track sales, manage attendees, and more.',
    },
    {
      icon: Users,
      title: 'Seamless Ticketing',
      description:
        'Sell tickets instantly with multiple pricing tiers, QR code check-in, and real-time analytics.',
    },
    {
      icon: Sparkles,
      title: 'Beautiful Event Pages',
      description: 'Stunning event pages that showcase your event and drive ticket sales.',
    },
  ];

  return (
    <Layout>
      <section className="gradient-hero relative overflow-hidden py-24 lg:py-32">
        <div className="absolute inset-0 opacity-20">
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_30%_50%,rgba(255,255,255,0.1),transparent_50%)]" />
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_70%_80%,rgba(255,107,107,0.2),transparent_40%)]" />
        </div>
        <div className="container relative">
          <div className="mx-auto max-w-3xl text-center">
            <h1 className="animate-fade-in mb-6 text-4xl font-extrabold text-white md:text-5xl lg:text-6xl">
              Discover & Create <span className="text-primary">Unforgettable</span> Events
            </h1>
            <p
              className="animate-fade-in mb-8 text-lg text-white/80 md:text-xl"
              style={{ animationDelay: '0.1s' }}
            >
              Join thousands of organizers and attendees. Find your next experience or create events
              that inspire.
            </p>
            <div
              className="animate-fade-in flex flex-col justify-center gap-4 sm:flex-row"
              style={{ animationDelay: '0.2s' }}
            >
              <Button size="lg" asChild className="text-base">
                <Link to="/events">
                  Browse Events
                  <ArrowRight className="ml-2 h-5 w-5" />
                </Link>
              </Button>
              {!isAuthenticated && (
                <Button
                  size="lg"
                  variant="outline"
                  className="border-white/20 bg-white/10 text-base text-white hover:bg-white/20"
                  onClick={login}
                >
                  Host an Event
                </Button>
              )}
              {isAuthenticated && user?.role === 'ORGANIZER' && (
                <Button
                  size="lg"
                  variant="outline"
                  asChild
                  className="border-white/20 bg-white/10 text-base text-white hover:bg-white/20"
                >
                  <Link to="/organizer/events/new">Create Event</Link>
                </Button>
              )}
            </div>
          </div>
        </div>
      </section>

      <section className="border-b bg-card py-12">
        <div className="container">
          <div className="grid grid-cols-2 gap-8 md:grid-cols-4">
            {stats.map(stat => (
              <div key={stat.label} className="text-center">
                <p className="text-3xl font-bold text-primary md:text-4xl">{stat.value}</p>
                <p className="mt-1 text-sm text-muted-foreground">{stat.label}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="py-16 lg:py-24">
        <div className="container">
          <div className="mb-8 flex items-center justify-between">
            <div>
              <h2 className="text-2xl font-bold md:text-3xl">Featured Events</h2>
              <p className="mt-1 text-muted-foreground">Don't miss out on these popular events</p>
            </div>
            <Button variant="ghost" asChild>
              <Link to="/events">
                View All
                <ArrowRight className="ml-2 h-4 w-4" />
              </Link>
            </Button>
          </div>

          {isLoading ? (
            <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
              {[1, 2, 3].map(i => (
                <Skeleton key={i} className="h-80 w-full rounded-lg" />
              ))}
            </div>
          ) : featuredEvents.length > 0 ? (
            <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
              {featuredEvents.map((event, index) => (
                <div
                  key={event.id}
                  className="animate-slide-up"
                  style={{ animationDelay: `${index * 0.1}s` }}
                >
                  <EventCard event={event} />
                </div>
              ))}
            </div>
          ) : (
            <div className="py-12 text-center">
              <p className="text-muted-foreground">No featured events available.</p>
              <Button asChild variant="link" className="mt-2">
                <Link to="/events">Browse all events</Link>
              </Button>
            </div>
          )}
        </div>
      </section>

      <section className="bg-muted/50 py-16 lg:py-24">
        <div className="container">
          <div className="mb-12 text-center">
            <h2 className="text-2xl font-bold md:text-3xl">Explore by Category</h2>
            <p className="mt-2 text-muted-foreground">Find events that match your interests</p>
          </div>

          <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
            {[
              'Music',
              'Sports',
              'Technology',
              'Arts',
              'Business',
              'Food',
              'Community',
              'Education',
            ].map(category => (
              <Link
                key={category}
                to={`/events?category=${category}`}
                className="group relative overflow-hidden rounded-xl bg-card p-6 text-center transition-all hover:-translate-y-1 hover:shadow-lg"
              >
                <div className="gradient-primary absolute inset-0 opacity-0 transition-opacity group-hover:opacity-10" />
                <h3 className="font-semibold transition-colors group-hover:text-primary">
                  {category}
                </h3>
              </Link>
            ))}
          </div>
        </div>
      </section>

      <section className="py-16 lg:py-24">
        <div className="container">
          <div className="mb-12 text-center">
            <h2 className="text-2xl font-bold md:text-3xl">Everything You Need to Host</h2>
            <p className="mt-2 text-muted-foreground">
              Powerful tools for event organizers of all sizes
            </p>
          </div>

          <div className="grid gap-8 md:grid-cols-3">
            {features.map((feature, index) => (
              <div
                key={feature.title}
                className="animate-slide-up relative rounded-2xl border bg-card p-6 transition-all hover:-translate-y-1 hover:shadow-lg"
                style={{ animationDelay: `${index * 0.1}s` }}
              >
                <div className="gradient-primary mb-4 flex h-12 w-12 items-center justify-center rounded-xl">
                  <feature.icon className="h-6 w-6 text-primary-foreground" />
                </div>
                <h3 className="mb-2 text-lg font-semibold">{feature.title}</h3>
                <p className="text-sm text-muted-foreground">{feature.description}</p>
              </div>
            ))}
          </div>

          {!isAuthenticated && (
            <div className="mt-12 text-center">
              <Button size="lg" onClick={login}>
                Start Hosting Events
                <ArrowRight className="ml-2 h-5 w-5" />
              </Button>
            </div>
          )}
        </div>
      </section>

      <section className="bg-muted/50 py-16 lg:py-24">
        <div className="container">
          <div className="mb-8 flex items-center justify-between">
            <div>
              <h2 className="text-2xl font-bold md:text-3xl">Upcoming Events</h2>
              <p className="mt-1 text-muted-foreground">Discover what's happening soon</p>
            </div>
            <Button variant="ghost" asChild>
              <Link to="/events">
                See More
                <ArrowRight className="ml-2 h-4 w-4" />
              </Link>
            </Button>
          </div>

          {isLoading ? (
            <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
              {[1, 2, 3].map(i => (
                <Skeleton key={i} className="h-80 w-full rounded-lg" />
              ))}
            </div>
          ) : upcomingEvents.length > 0 ? (
            <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
              {upcomingEvents.map((event, index) => (
                <div
                  key={event.id}
                  className="animate-slide-up"
                  style={{ animationDelay: `${index * 0.05}s` }}
                >
                  <EventCard event={event} />
                </div>
              ))}
            </div>
          ) : (
            <div className="py-12 text-center">
              <p className="text-muted-foreground">No upcoming events available.</p>
              <Button asChild variant="link" className="mt-2">
                <Link to="/events">Browse all events</Link>
              </Button>
            </div>
          )}
        </div>
      </section>

      <section className="py-16 lg:py-24">
        <div className="container">
          <div className="gradient-primary relative overflow-hidden rounded-3xl p-8 text-center md:p-12 lg:p-16">
            <div className="absolute inset-0 opacity-20">
              <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_50%,rgba(255,255,255,0.3),transparent_60%)]" />
            </div>
            <div className="relative">
              <h2 className="mb-4 text-2xl font-bold text-primary-foreground md:text-3xl lg:text-4xl">
                Ready to Create Your Next Event?
              </h2>
              <p className="mx-auto mb-8 max-w-2xl text-primary-foreground/80">
                Join thousands of successful organizers. Create stunning event pages, sell tickets,
                and grow your audience.
              </p>
              <div className="flex flex-col justify-center gap-4 sm:flex-row">
                <Button
                  size="lg"
                  variant="secondary"
                  onClick={() => !isAuthenticated && login()}
                  asChild={isAuthenticated}
                  className="bg-white text-primary hover:bg-white/90"
                >
                  {isAuthenticated ? (
                    <Link to="/organizer/events/new">Create Your Event</Link>
                  ) : (
                    <span>Get Started Free</span>
                  )}
                </Button>
              </div>
              <div className="mt-8 flex flex-wrap justify-center gap-6 text-sm text-primary-foreground/80">
                <span className="flex items-center gap-2">
                  <CheckCircle className="h-4 w-4" />
                  No setup fees
                </span>
                <span className="flex items-center gap-2">
                  <CheckCircle className="h-4 w-4" />
                  Instant payouts
                </span>
                <span className="flex items-center gap-2">
                  <CheckCircle className="h-4 w-4" />
                  24/7 support
                </span>
              </div>
            </div>
          </div>
        </div>
      </section>
    </Layout>
  );
};

export default Index;
