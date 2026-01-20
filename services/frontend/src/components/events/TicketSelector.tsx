import { useState } from 'react';
import { TicketType, EventType } from '@/types';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { useCart } from '@/contexts/CartContext';
import { toast } from 'sonner';
import { Minus, Plus, Ticket } from 'lucide-react';

interface TicketSelectorProps {
  ticketTypes: TicketType[];
  event: EventType;
}

export function TicketSelector({ ticketTypes, event }: TicketSelectorProps) {
  const { addToCart } = useCart();
  const [quantities, setQuantities] = useState<Record<string, number>>(
    ticketTypes.reduce((acc, t) => ({ ...acc, [String(t.id)]: 0 }), {})
  );

  const updateQuantity = (ticketId: string | number, delta: number) => {
    const strId = String(ticketId);
    setQuantities(prev => {
      const ticket = ticketTypes.find(t => String(t.id) === strId);
      if (!ticket) return prev;

      const available = ticket.remaining ?? ticket.capacity;
      const newQty = Math.max(0, Math.min(prev[strId] + delta, available, 10));
      return { ...prev, [strId]: newQty };
    });
  };

  const handleAddToCart = () => {
    console.log('[TicketSelector] handleAddToCart called');
    console.log('[TicketSelector] quantities:', quantities);
    console.log(
      '[TicketSelector] ticketTypes:',
      ticketTypes.map(t => ({ id: String(t.id), name: t.name, price: t.price }))
    );
    console.log('[TicketSelector] event:', event);

    const selectedTickets = Object.entries(quantities).filter(([_, qty]) => qty > 0);
    console.log('[TicketSelector] selectedTickets:', selectedTickets);

    if (selectedTickets.length === 0) {
      toast.error('Please select at least one ticket');
      return;
    }

    selectedTickets.forEach(([ticketId, qty]) => {
      const ticketType = ticketTypes.find(t => String(t.id) === ticketId);
      console.log('[TicketSelector] Looking for ticketId:', ticketId, 'Found:', ticketType);
      if (ticketType) {
        console.log('[TicketSelector] Calling addToCart with:', {
          ticketId: ticketType.id,
          qty,
          eventId: event.id,
        });
        addToCart(ticketType, qty, event);
      }
    });

    toast.success('Tickets added to cart!');
    setQuantities(ticketTypes.reduce((acc, t) => ({ ...acc, [String(t.id)]: 0 }), {}));
  };

  const totalSelected = Object.values(quantities).reduce((sum, qty) => sum + qty, 0);
  const totalPrice = ticketTypes.reduce(
    (sum, t) => sum + t.price * (quantities[String(t.id)] || 0),
    0
  );

  return (
    <div className="space-y-4">
      <h3 className="flex items-center gap-2 text-lg font-semibold">
        <Ticket className="h-5 w-5 text-primary" />
        Select Tickets
      </h3>

      <div className="space-y-3">
        {ticketTypes.map(ticket => {
          const strId = String(ticket.id);
          const available = ticket.remaining ?? ticket.capacity;
          return (
            <Card key={strId} className="overflow-hidden">
              <CardContent className="p-4">
                <div className="flex items-center justify-between gap-4">
                  <div className="min-w-0 flex-1">
                    <h4 className="font-medium">{ticket.name}</h4>
                    <p className="text-sm text-muted-foreground">{available} available</p>
                  </div>

                  <div className="text-right">
                    <p className="text-lg font-bold">
                      {ticket.price === 0 ? 'Free' : `$${ticket.price}`}
                    </p>
                  </div>

                  <div className="flex items-center gap-2">
                    <Button
                      variant="outline"
                      size="icon"
                      className="h-8 w-8"
                      onClick={() => updateQuantity(ticket.id, -1)}
                      disabled={quantities[strId] === 0}
                    >
                      <Minus className="h-4 w-4" />
                    </Button>
                    <span className="w-8 text-center font-medium">{quantities[strId]}</span>
                    <Button
                      variant="outline"
                      size="icon"
                      className="h-8 w-8"
                      onClick={() => updateQuantity(ticket.id, 1)}
                      disabled={quantities[strId] >= available || quantities[strId] >= 10}
                    >
                      <Plus className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>

      {totalSelected > 0 && (
        <div className="border-t pt-4">
          <div className="mb-4 flex items-center justify-between">
            <span className="text-muted-foreground">
              {totalSelected} ticket{totalSelected > 1 ? 's' : ''}
            </span>
            <span className="text-xl font-bold">${totalPrice.toFixed(2)}</span>
          </div>
          <Button onClick={handleAddToCart} className="w-full" size="lg">
            Add to Cart
          </Button>
        </div>
      )}
    </div>
  );
}
