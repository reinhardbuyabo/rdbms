import { Link } from 'react-router-dom';
import { EventType, TicketType } from '@/types';
import { Card, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { CalendarDays, MapPin, Users } from 'lucide-react';
import { format } from 'date-fns';

interface EventCardProps {
  event: EventType;
  variant?: 'default' | 'compact';
}

function formatEventDate(dateStr: string | undefined): string {
  if (!dateStr) return 'Date TBD';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return 'Date TBD';
  return format(date, 'MMM d, yyyy');
}

function formatEventDateTime(dateStr: string | undefined): string {
  if (!dateStr) return 'Date TBD';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return 'Date TBD';
  return format(date, 'EEEE, MMMM d, yyyy');
}

export function EventCard({ event, variant = 'default' }: EventCardProps) {
  const ticketTypes: TicketType[] = event.ticket_types || event.ticketTypes || [];

  const lowestPrice =
    ticketTypes.length > 0
      ? ticketTypes.reduce(
          (min: number, t: TicketType) => Math.min(min, t.price || 0),
          ticketTypes[0].price || 0
        )
      : undefined;

  const totalRemaining = ticketTypes.reduce(
    (sum: number, t: TicketType) => sum + (t.remaining ?? t.capacity),
    0
  );

  const statusColors: Record<string, string> = {
    DRAFT: 'status-draft',
    PUBLISHED: 'status-published',
    CANCELLED: 'status-cancelled',
  };

  if (variant === 'compact') {
    return (
      <Link to={`/events/${event.id}`}>
        <Card className="card-hover group overflow-hidden">
          <div className="flex gap-4 p-4">
            <div className="min-w-0 flex-1">
              <h3 className="truncate font-semibold">{event.title}</h3>
              <p className="mt-1 flex items-center gap-1 text-sm text-muted-foreground">
                <CalendarDays className="h-3.5 w-3.5" />
                {formatEventDate(event.start_time)}
              </p>
              <p className="flex items-center gap-1 text-sm text-muted-foreground">
                <MapPin className="h-3.5 w-3.5" />
                <span className="truncate">{event.venue}</span>
              </p>
            </div>
          </div>
        </Card>
      </Link>
    );
  }

  return (
    <Link to={`/events/${event.id}`}>
      <Card className="card-hover group h-full overflow-hidden">
        <div className="relative aspect-[16/10] overflow-hidden bg-muted">
          <div className="flex h-full w-full items-center justify-center bg-gradient-to-br from-primary/10 to-primary/5">
            <CalendarDays className="h-12 w-12 text-muted-foreground/30" />
          </div>
          <div className="absolute inset-0 bg-gradient-to-t from-black/60 via-transparent to-transparent" />
          {event.status !== 'PUBLISHED' && (
            <div className="absolute left-3 top-3 flex gap-2">
              <span className={`status-badge ${statusColors[event.status] || 'status-draft'}`}>
                {event.status}
              </span>
            </div>
          )}
          <div className="absolute bottom-3 left-3 right-3">
            <p className="flex items-center gap-1 text-xs text-white/80">
              <CalendarDays className="h-3.5 w-3.5" />
              {formatEventDateTime(event.start_time)}
            </p>
          </div>
        </div>
        <CardContent className="p-4">
          <h3 className="mb-2 line-clamp-2 text-lg font-semibold transition-colors group-hover:text-primary">
            {event.title}
          </h3>
          <p className="mb-3 flex items-center gap-1 text-sm text-muted-foreground">
            <MapPin className="h-4 w-4 flex-shrink-0" />
            <span className="truncate">{event.venue || 'Location TBD'}</span>
          </p>
          <div className="flex items-center justify-between">
            <div>
              <p className="text-lg font-bold text-primary">
                {lowestPrice === undefined
                  ? 'Free'
                  : lowestPrice === 0
                    ? 'Free'
                    : `From $${lowestPrice}`}
              </p>
            </div>
            {ticketTypes.length > 0 && totalRemaining > 0 && totalRemaining < 100 && (
              <p className="flex items-center gap-1 text-xs text-muted-foreground">
                <Users className="h-3.5 w-3.5" />
                {totalRemaining} left
              </p>
            )}
          </div>
        </CardContent>
      </Card>
    </Link>
  );
}
