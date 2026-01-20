import { useParams, Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Layout } from '@/components/layout/Layout';
import { TicketSelector } from '@/components/events/TicketSelector';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';
import { apiClient } from '@/lib/api';
import { format } from 'date-fns';
import { CalendarDays, Clock, MapPin, Share2, Heart, ArrowLeft, User } from 'lucide-react';
import { toast } from 'sonner';

function formatEventDate(dateStr: string | undefined): string {
  if (!dateStr) return 'Date TBD';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return 'Date TBD';
  return format(date, 'EEEE, MMMM d, yyyy');
}

function formatEventTime(dateStr: string | undefined): string {
  if (!dateStr) return 'Time TBD';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return 'Time TBD';
  return format(date, 'h:mm a');
}

const EventDetailPage = () => {
  const { eventId } = useParams();

  const {
    data: eventData,
    isLoading,
    error,
  } = useQuery({
    queryKey: ['event', eventId],
    queryFn: () => apiClient.getEvent(eventId!),
    enabled: !!eventId,
  });

  const event = eventData?.event;
  const ticketTypes = eventData?.ticket_types || [];
  const organizerName = eventData?.organizer_name || 'Event Organizer';

  if (isLoading) {
    return (
      <Layout>
        <div className="relative h-[40vh] overflow-hidden bg-muted md:h-[50vh]">
          <Skeleton className="h-full w-full" />
        </div>
        <div className="container relative -mt-24 pb-16">
          <div className="grid gap-8 lg:grid-cols-3">
            <div className="space-y-8 lg:col-span-2">
              <Skeleton className="h-64 w-full rounded-2xl" />
              <Skeleton className="h-32 w-full rounded-2xl" />
            </div>
            <div className="lg:col-span-1">
              <Skeleton className="h-64 w-full rounded-2xl" />
            </div>
          </div>
        </div>
      </Layout>
    );
  }

  if (error || !event) {
    return (
      <Layout>
        <div className="container py-16 text-center">
          <h1 className="mb-4 text-2xl font-bold">Event Not Found</h1>
          <p className="mb-8 text-muted-foreground">
            The event you're looking for doesn't exist or has been removed.
          </p>
          <Button asChild>
            <Link to="/events">Browse Events</Link>
          </Button>
        </div>
      </Layout>
    );
  }

  const handleShare = async () => {
    try {
      await navigator.share({
        title: event.title,
        text: event.description,
        url: window.location.href,
      });
    } catch {
      navigator.clipboard.writeText(window.location.href);
      toast.success('Link copied to clipboard!');
    }
  };

  const handleSave = () => {
    toast.success('Event saved to your favorites!');
  };

  return (
    <Layout>
      <div className="relative h-[40vh] overflow-hidden bg-muted md:h-[50vh]">
        <div className="flex h-full w-full items-center justify-center bg-gradient-to-br from-primary/10 to-primary/5">
          <CalendarDays className="h-16 w-16 text-muted-foreground/20" />
        </div>
        <div className="absolute inset-0 bg-gradient-to-t from-background via-background/40 to-transparent" />

        <div className="absolute left-4 top-4">
          <Button variant="ghost" size="sm" asChild className="bg-background/80 backdrop-blur-sm">
            <Link to="/events">
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Link>
          </Button>
        </div>

        <div className="absolute right-4 top-4 flex gap-2">
          <Button
            variant="ghost"
            size="icon"
            onClick={handleSave}
            className="bg-background/80 backdrop-blur-sm"
          >
            <Heart className="h-5 w-5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={handleShare}
            className="bg-background/80 backdrop-blur-sm"
          >
            <Share2 className="h-5 w-5" />
          </Button>
        </div>
      </div>

      <div className="container relative -mt-24 pb-16">
        <div className="grid gap-8 lg:grid-cols-3">
          <div className="space-y-8 lg:col-span-2">
            <div className="rounded-2xl bg-card p-6 shadow-lg md:p-8">
              {event.status !== 'PUBLISHED' && (
                <div className="mb-4 flex flex-wrap gap-2">
                  <Badge variant="outline">{event.status}</Badge>
                </div>
              )}

              <h1 className="mb-6 text-2xl font-bold md:text-3xl lg:text-4xl">{event.title}</h1>

              <div className="mb-8 grid gap-4 sm:grid-cols-2">
                <div className="flex items-start gap-3">
                  <div className="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg bg-primary/10">
                    <CalendarDays className="h-5 w-5 text-primary" />
                  </div>
                  <div>
                    <p className="font-medium">{formatEventDate(event.start_time)}</p>
                    <p className="text-sm text-muted-foreground">
                      {formatEventTime(event.start_time)} - {formatEventTime(event.end_time)}
                    </p>
                  </div>
                </div>

                <div className="flex items-start gap-3">
                  <div className="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg bg-primary/10">
                    <MapPin className="h-5 w-5 text-primary" />
                  </div>
                  <div>
                    <p className="font-medium">Venue</p>
                    <p className="text-sm text-muted-foreground">{event.venue || 'TBA'}</p>
                    {event.location && (
                      <p className="text-xs text-muted-foreground">{event.location}</p>
                    )}
                  </div>
                </div>
              </div>

              <div className="prose prose-sm max-w-none">
                <h3 className="mb-3 text-lg font-semibold">About This Event</h3>
                <p className="leading-relaxed text-muted-foreground">
                  {event.description || 'No description available.'}
                </p>
              </div>
            </div>

            <div className="rounded-2xl bg-card p-6 shadow-lg">
              <h3 className="mb-4 text-lg font-semibold">Organized by</h3>
              <div className="flex items-center gap-4">
                <div className="gradient-primary flex h-12 w-12 items-center justify-center rounded-full">
                  <User className="h-6 w-6 text-primary-foreground" />
                </div>
                <div>
                  <p className="font-medium">{organizerName}</p>
                  <p className="text-sm text-muted-foreground">Verified Organizer</p>
                </div>
              </div>
            </div>
          </div>

          <div className="lg:col-span-1">
            <div className="sticky top-24 rounded-2xl bg-card p-6 shadow-lg">
              {ticketTypes && ticketTypes.length > 0 ? (
                <TicketSelector ticketTypes={ticketTypes} event={event} />
              ) : (
                <div className="py-8 text-center">
                  <p className="text-muted-foreground">
                    Tickets are not available for this event yet.
                  </p>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </Layout>
  );
};

export default EventDetailPage;
