import { Link, useLocation } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { useAuth } from '@/contexts/AuthContext';
import { useCart } from '@/contexts/CartContext';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import { Badge } from '@/components/ui/badge';
import { CalendarDays, ShoppingCart, LogOut, LayoutDashboard, Ticket, Menu } from 'lucide-react';
import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet';
import { useState } from 'react';
import { toast } from 'sonner';

export function Navbar() {
  const { user, isAuthenticated, login, logout, isLoading, mockLogin } = useAuth();
  const { totalItems } = useCart();
  const location = useLocation();
  const [mobileOpen, setMobileOpen] = useState(false);

  const isOrganizer = user?.role === 'ORGANIZER';
  const isOrganizerRoute = location.pathname.startsWith('/organizer');

  const handleMockLogin = async (email: string) => {
    try {
      await mockLogin(email);
      toast.success(`Logged in as ${email}`);
      setMobileOpen(false);
    } catch {
      toast.error('Mock login failed');
    }
  };

  const navLinks = isOrganizer
    ? [
        { href: '/organizer/dashboard', label: 'Dashboard' },
        { href: '/organizer/events', label: 'My Events' },
        { href: '/events', label: 'Browse Events' },
      ]
    : [
        { href: '/events', label: 'Browse Events' },
        { href: '/my-tickets', label: 'My Tickets' },
      ];

  return (
    <header className="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="container flex h-16 items-center justify-between">
        <div className="flex items-center gap-8">
          <Link to="/" className="flex items-center gap-2">
            <div className="gradient-primary flex h-9 w-9 items-center justify-center rounded-lg">
              <CalendarDays className="h-5 w-5 text-primary-foreground" />
            </div>
            <span className="text-xl font-bold">Eventify</span>
          </Link>

          <nav className="hidden items-center gap-6 md:flex">
            {navLinks.map(link => (
              <Link
                key={link.href}
                to={link.href}
                className={`text-sm font-medium transition-colors hover:text-primary ${
                  location.pathname === link.href ? 'text-primary' : 'text-muted-foreground'
                }`}
              >
                {link.label}
              </Link>
            ))}
          </nav>
        </div>

        <div className="flex items-center gap-4">
          {!isOrganizer && (
            <Link to="/cart" className="relative">
              <Button variant="ghost" size="icon">
                <ShoppingCart className="h-5 w-5" />
                {totalItems > 0 && (
                  <Badge className="absolute -right-1 -top-1 flex h-5 w-5 items-center justify-center rounded-full p-0 text-xs">
                    {totalItems}
                  </Badge>
                )}
              </Button>
            </Link>
          )}

          {isLoading ? (
            <div className="h-9 w-9 animate-pulse rounded-full bg-muted" />
          ) : isAuthenticated ? (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" className="relative h-9 w-9 rounded-full">
                  <Avatar className="h-9 w-9">
                    <AvatarImage src={user?.avatarUrl} alt={user?.name} />
                    <AvatarFallback className="gradient-primary text-primary-foreground">
                      {user?.name?.charAt(0) || user?.email?.charAt(0)}
                    </AvatarFallback>
                  </Avatar>
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent className="w-56" align="end" forceMount>
                <div className="flex items-center gap-2 p-2">
                  <Avatar className="h-8 w-8">
                    <AvatarFallback className="gradient-primary text-xs text-primary-foreground">
                      {user?.name?.charAt(0) || user?.email?.charAt(0)}
                    </AvatarFallback>
                  </Avatar>
                  <div className="flex flex-col space-y-0.5">
                    <p className="text-sm font-medium">{user?.name}</p>
                    <p className="text-xs text-muted-foreground">{user?.email}</p>
                  </div>
                </div>
                <DropdownMenuSeparator />
                {isOrganizer ? (
                  <>
                    <DropdownMenuItem asChild>
                      <Link to="/organizer/dashboard" className="cursor-pointer">
                        <LayoutDashboard className="mr-2 h-4 w-4" />
                        Dashboard
                      </Link>
                    </DropdownMenuItem>
                    <DropdownMenuItem asChild>
                      <Link to="/organizer/events" className="cursor-pointer">
                        <CalendarDays className="mr-2 h-4 w-4" />
                        My Events
                      </Link>
                    </DropdownMenuItem>
                  </>
                ) : (
                  <>
                    <DropdownMenuItem asChild>
                      <Link to="/my-tickets" className="cursor-pointer">
                        <Ticket className="mr-2 h-4 w-4" />
                        My Tickets
                      </Link>
                    </DropdownMenuItem>
                  </>
                )}
                <DropdownMenuSeparator />
                <DropdownMenuItem onClick={logout} className="cursor-pointer text-destructive">
                  <LogOut className="mr-2 h-4 w-4" />
                  Log out
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : (
            <div className="hidden items-center gap-2 md:flex">
              <Button variant="ghost" onClick={login}>
                Sign In
              </Button>
              <Button onClick={login}>Host Event</Button>
              {import.meta.env?.VITE_USE_MOCK_AUTH === 'true' && (
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="outline" size="sm">
                      Demo
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem onClick={() => handleMockLogin('test@example.com')}>
                      Sign in as Customer
                    </DropdownMenuItem>
                    <DropdownMenuItem onClick={() => handleMockLogin('organizer@example.com')}>
                      Sign in as Organizer
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              )}
            </div>
          )}

          {/* Mobile Menu */}
          <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
            <SheetTrigger asChild className="md:hidden">
              <Button variant="ghost" size="icon">
                <Menu className="h-5 w-5" />
              </Button>
            </SheetTrigger>
            <SheetContent side="right" className="w-[300px] sm:w-[400px]">
              <nav className="mt-8 flex flex-col gap-4">
                {navLinks.map(link => (
                  <Link
                    key={link.href}
                    to={link.href}
                    onClick={() => setMobileOpen(false)}
                    className={`text-lg font-medium transition-colors hover:text-primary ${
                      location.pathname === link.href ? 'text-primary' : 'text-muted-foreground'
                    }`}
                  >
                    {link.label}
                  </Link>
                ))}
                {!isAuthenticated && (
                  <>
                    <Button
                      variant="ghost"
                      onClick={() => {
                        login();
                        setMobileOpen(false);
                      }}
                      className="justify-start"
                    >
                      Sign In with Google
                    </Button>
                    <Button
                      variant="outline"
                      onClick={() => handleMockLogin('test@example.com')}
                      className="justify-start"
                    >
                      Demo: Sign in as Customer
                    </Button>
                    <Button
                      variant="outline"
                      onClick={() => handleMockLogin('organizer@example.com')}
                      className="justify-start"
                    >
                      Demo: Sign in as Organizer
                    </Button>
                  </>
                )}
              </nav>
            </SheetContent>
          </Sheet>
        </div>
      </div>
    </header>
  );
}
