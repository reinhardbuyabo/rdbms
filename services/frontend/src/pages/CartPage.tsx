import { Link } from 'react-router-dom';
import { Layout } from '@/components/layout/Layout';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { useCart } from '@/contexts/CartContext';
import { Minus, Plus, Trash2, ShoppingBag, ArrowRight } from 'lucide-react';
import { format } from 'date-fns';

function formatEventDate(dateStr: string | undefined): string {
  if (!dateStr) return 'Date TBD';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return 'Date TBD';
  return format(date, 'MMM d, yyyy');
}

const CartPage = () => {
  const { items, updateQuantity, removeFromCart, totalAmount, clearCart } = useCart();

  console.log('[CartPage] Rendering with items:', items.length);

  if (items.length === 0) {
    return (
      <Layout>
        <div className="container py-16 text-center">
          <div className="mx-auto max-w-md">
            <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-muted">
              <ShoppingBag className="h-10 w-10 text-muted-foreground" />
            </div>
            <h1 className="mb-2 text-2xl font-bold">Your cart is empty</h1>
            <p className="mb-8 text-muted-foreground">
              Looks like you haven't added any tickets yet. Browse our events to find something
              you'll love.
            </p>
            <Button asChild size="lg">
              <Link to="/events">
                Browse Events
                <ArrowRight className="ml-2 h-5 w-5" />
              </Link>
            </Button>
          </div>
        </div>
      </Layout>
    );
  }

  const serviceFee = totalAmount * 0.05;
  const grandTotal = totalAmount + serviceFee;

  return (
    <Layout>
      <div className="container py-8 lg:py-12">
        <div className="mb-8">
          <h1 className="mb-2 text-3xl font-bold">Your Cart</h1>
          <p className="text-muted-foreground">Review your tickets before checkout</p>
        </div>

        <div className="grid gap-8 lg:grid-cols-3">
          {/* Cart Items */}
          <div className="space-y-4 lg:col-span-2">
            {items.map(item => (
              <Card key={String(item.ticketType.id)} className="overflow-hidden">
                <CardContent className="p-0">
                  <div className="flex gap-4">
                    <div className="flex h-32 w-32 flex-shrink-0 items-center justify-center bg-muted">
                      <ShoppingBag className="h-8 w-8 text-muted-foreground" />
                    </div>
                    <div className="flex-1 py-4 pr-4">
                      <div className="flex items-start justify-between gap-4">
                        <div className="min-w-0">
                          <Link
                            to={`/events/${item.event.id}`}
                            className="line-clamp-1 font-semibold transition-colors hover:text-primary"
                          >
                            {item.event.title}
                          </Link>
                          <p className="mt-1 text-sm text-muted-foreground">
                            {formatEventDate(item.event.start_time)}
                          </p>
                          <p className="mt-2 text-sm font-medium text-primary">
                            {item.ticketType.name}
                          </p>
                        </div>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => removeFromCart(String(item.ticketType.id))}
                          className="text-muted-foreground hover:text-destructive"
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>

                      <div className="mt-4 flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <Button
                            variant="outline"
                            size="icon"
                            className="h-8 w-8"
                            onClick={() =>
                              updateQuantity(String(item.ticketType.id), item.quantity - 1)
                            }
                          >
                            <Minus className="h-4 w-4" />
                          </Button>
                          <span className="w-8 text-center font-medium">{item.quantity}</span>
                          <Button
                            variant="outline"
                            size="icon"
                            className="h-8 w-8"
                            onClick={() =>
                              updateQuantity(String(item.ticketType.id), item.quantity + 1)
                            }
                            disabled={
                              item.quantity >=
                              (item.ticketType.remaining || item.ticketType.capacity)
                            }
                          >
                            <Plus className="h-4 w-4" />
                          </Button>
                        </div>
                        <p className="font-bold">
                          ${((item.ticketType.price || 0) * item.quantity).toFixed(2)}
                        </p>
                      </div>
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}

            <div className="flex justify-end">
              <Button variant="ghost" onClick={clearCart} className="text-muted-foreground">
                Clear Cart
              </Button>
            </div>
          </div>

          {/* Order Summary */}
          <div className="lg:col-span-1">
            <Card className="sticky top-24">
              <CardContent className="p-6">
                <h2 className="mb-4 text-lg font-semibold">Order Summary</h2>

                <div className="space-y-3 text-sm">
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Subtotal</span>
                    <span>${totalAmount.toFixed(2)}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Service Fee</span>
                    <span>${serviceFee.toFixed(2)}</span>
                  </div>
                  <div className="flex justify-between border-t pt-3 text-lg font-bold">
                    <span>Total</span>
                    <span>${grandTotal.toFixed(2)}</span>
                  </div>
                </div>

                <Button asChild className="mt-6 w-full" size="lg">
                  <Link to="/checkout">
                    Proceed to Checkout
                    <ArrowRight className="ml-2 h-5 w-5" />
                  </Link>
                </Button>

                <p className="mt-4 text-center text-xs text-muted-foreground">
                  Secure checkout powered by Stripe
                </p>
              </CardContent>
            </Card>
          </div>
        </div>
      </div>
    </Layout>
  );
};

export default CartPage;
