import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Layout } from '@/components/layout/Layout';
import { EventCard } from '@/components/events/EventCard';
import { EventFilters } from '@/components/events/EventFilters';
import { apiClient } from '@/lib/api';
import { useSearchParams } from 'react-router-dom';
import { Skeleton } from '@/components/ui/skeleton';

function getEventTime(dateStr: string | undefined): number {
  if (!dateStr) return Infinity;
  const date = new Date(dateStr);
  return isNaN(date.getTime()) ? Infinity : date.getTime();
}

const EventsPage = () => {
  const [searchParams] = useSearchParams();
  const [searchQuery, setSearchQuery] = useState(searchParams.get('search') || '');
  const [selectedCategory, setSelectedCategory] = useState(searchParams.get('category') || '');
  const [sortBy, setSortBy] = useState('date');

  const { data, isLoading, error } = useQuery({
    queryKey: ['events'],
    queryFn: () => apiClient.listEvents({ status: 'PUBLISHED' }),
  });

  const filteredEvents = useMemo(() => {
    const events = data?.data || [];
    let result = [...events];

    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      result = result.filter(
        e =>
          e.title.toLowerCase().includes(query) ||
          e.description?.toLowerCase().includes(query) ||
          e.venue?.toLowerCase().includes(query)
      );
    }

    if (selectedCategory && selectedCategory !== 'all') {
      result = result.filter(_e => {
        // Category filtering not currently implemented in API
        return true;
      });
    }

    switch (sortBy) {
      case 'date':
        result.sort((a, b) => getEventTime(a.start_time) - getEventTime(b.start_time));
        break;
      case 'price-low':
        result.sort((a, b) => {
          const aTicketTypes = a.ticketTypes || [];
          const bTicketTypes = b.ticketTypes || [];
          const aPrices = aTicketTypes.map(t => t.price);
          const bPrices = bTicketTypes.map(t => t.price);
          const aMin = aPrices.length > 0 ? Math.min(...aPrices) : 0;
          const bMin = bPrices.length > 0 ? Math.min(...bPrices) : 0;
          return aMin - bMin;
        });
        break;
      case 'price-high':
        result.sort((a, b) => {
          const aTicketTypes = a.ticketTypes || [];
          const bTicketTypes = b.ticketTypes || [];
          const aPrices = aTicketTypes.map(t => t.price);
          const bPrices = bTicketTypes.map(t => t.price);
          const aMax = aPrices.length > 0 ? Math.max(...aPrices) : 0;
          const bMax = bPrices.length > 0 ? Math.max(...bPrices) : 0;
          return bMax - aMax;
        });
        break;
    }

    return result;
  }, [data, searchQuery, selectedCategory, sortBy]);

  if (error) {
    return (
      <Layout>
        <div className="container py-16 text-center">
          <h2 className="mb-4 text-2xl font-bold">Failed to load events</h2>
          <p className="text-muted-foreground">Please try again later.</p>
        </div>
      </Layout>
    );
  }

  return (
    <Layout>
      <div className="container py-8 lg:py-12">
        <div className="mb-8">
          <h1 className="mb-2 text-3xl font-bold md:text-4xl">Browse Events</h1>
          <p className="text-muted-foreground">Discover amazing events happening near you</p>
        </div>

        <EventFilters
          searchQuery={searchQuery}
          setSearchQuery={setSearchQuery}
          selectedCategory={selectedCategory}
          setSelectedCategory={setSelectedCategory}
          sortBy={sortBy}
          setSortBy={setSortBy}
        />

        {isLoading ? (
          <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
            {[1, 2, 3, 4, 5, 6].map(i => (
              <div key={i} className="space-y-3">
                <Skeleton className="h-48 w-full rounded-lg" />
                <Skeleton className="h-4 w-3/4" />
                <Skeleton className="h-4 w-1/2" />
              </div>
            ))}
          </div>
        ) : filteredEvents.length === 0 ? (
          <div className="py-16 text-center">
            <p className="text-xl font-medium text-muted-foreground">No events found</p>
            <p className="mt-2 text-sm text-muted-foreground">
              Try adjusting your search or filter criteria
            </p>
          </div>
        ) : (
          <>
            <p className="mb-6 text-sm text-muted-foreground">
              Showing {filteredEvents.length} event{filteredEvents.length !== 1 ? 's' : ''}
            </p>
            <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
              {filteredEvents.map((event, index) => (
                <div
                  key={event.id}
                  className="animate-slide-up"
                  style={{ animationDelay: `${index * 0.05}s` }}
                >
                  <EventCard event={event} />
                </div>
              ))}
            </div>
          </>
        )}
      </div>
    </Layout>
  );
};

export default EventsPage;
