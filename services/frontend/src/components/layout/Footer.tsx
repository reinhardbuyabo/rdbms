import { Link } from 'react-router-dom';
import { CalendarDays, Mail, Phone, MapPin } from 'lucide-react';

export function Footer() {
  return (
    <footer className="border-t bg-secondary text-secondary-foreground">
      <div className="container py-12">
        <div className="grid grid-cols-1 gap-8 md:grid-cols-4">
          <div className="space-y-4">
            <Link to="/" className="flex items-center gap-2">
              <div className="gradient-primary flex h-9 w-9 items-center justify-center rounded-lg">
                <CalendarDays className="h-5 w-5 text-primary-foreground" />
              </div>
              <span className="text-xl font-bold">Eventify</span>
            </Link>
            <p className="text-sm text-secondary-foreground/80">
              Discover and create unforgettable events. Join thousands of organizers and attendees
              worldwide.
            </p>
          </div>

          <div>
            <h4 className="mb-4 font-semibold">For Attendees</h4>
            <ul className="space-y-2 text-sm text-secondary-foreground/80">
              <li>
                <Link to="/events" className="transition-colors hover:text-primary">
                  Browse Events
                </Link>
              </li>
              <li>
                <Link to="/my-tickets" className="transition-colors hover:text-primary">
                  My Tickets
                </Link>
              </li>
              <li>
                <Link to="/help" className="transition-colors hover:text-primary">
                  Help Center
                </Link>
              </li>
            </ul>
          </div>

          <div>
            <h4 className="mb-4 font-semibold">For Organizers</h4>
            <ul className="space-y-2 text-sm text-secondary-foreground/80">
              <li>
                <Link to="/organizer/dashboard" className="transition-colors hover:text-primary">
                  Dashboard
                </Link>
              </li>
              <li>
                <Link to="/organizer/events/new" className="transition-colors hover:text-primary">
                  Create Event
                </Link>
              </li>
              <li>
                <Link to="/pricing" className="transition-colors hover:text-primary">
                  Pricing
                </Link>
              </li>
            </ul>
          </div>

          <div>
            <h4 className="mb-4 font-semibold">Contact</h4>
            <ul className="space-y-2 text-sm text-secondary-foreground/80">
              <li className="flex items-center gap-2">
                <Mail className="h-4 w-4" />
                support@eventify.com
              </li>
              <li className="flex items-center gap-2">
                <Phone className="h-4 w-4" />
                +1 (555) 123-4567
              </li>
              <li className="flex items-center gap-2">
                <MapPin className="h-4 w-4" />
                San Francisco, CA
              </li>
            </ul>
          </div>
        </div>

        <div className="mt-8 border-t border-secondary-foreground/10 pt-8 text-center text-sm text-secondary-foreground/60">
          <p>&copy; {new Date().getFullYear()} Eventify. All rights reserved.</p>
        </div>
      </div>
    </footer>
  );
}
