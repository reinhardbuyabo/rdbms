import React, { createContext, useContext, useState, ReactNode, useEffect } from 'react';
import { CartItem, TicketType, EventType } from '@/types';

interface CartContextType {
  items: CartItem[];
  addToCart: (ticketType: TicketType, quantity: number, event: EventType) => void;
  removeFromCart: (ticketTypeId: string) => void;
  updateQuantity: (ticketTypeId: string, quantity: number) => void;
  clearCart: () => void;
  totalAmount: number;
  totalItems: number;
}

const CART_STORAGE_KEY = 'eventify_cart';

const CartContext = createContext<CartContextType | undefined>(undefined);

function ticketTypeIdEquals(a: string | number, b: string | number): boolean {
  return String(a) === String(b);
}

export function CartProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<CartItem[]>(() => {
    if (typeof window !== 'undefined') {
      try {
        const stored = localStorage.getItem(CART_STORAGE_KEY);
        console.log('[Cart] Loading from localStorage:', stored);
        if (stored) {
          const parsed = JSON.parse(stored);
          if (Array.isArray(parsed) && parsed.length > 0) {
            console.log('[Cart] Loaded items:', parsed.length);
            return parsed;
          }
        }
      } catch (e) {
        console.error('[Cart] Failed to load cart from storage:', e);
      }
    }
    return [];
  });

  useEffect(() => {
    console.log('[Cart] Saving to localStorage:', items.length, 'items');
    try {
      localStorage.setItem(CART_STORAGE_KEY, JSON.stringify(items));
    } catch (e) {
      console.error('[Cart] Failed to save cart to storage:', e);
    }
  }, [items]);

  const addToCart = (ticketType: TicketType, quantity: number, event: EventType) => {
    console.log('[Cart] Adding to cart:', ticketType.id, quantity, event.id);
    setItems(prev => {
      const existing = prev.find(item => ticketTypeIdEquals(item.ticketType.id, ticketType.id));
      let newItems;
      if (existing) {
        newItems = prev.map(item =>
          ticketTypeIdEquals(item.ticketType.id, ticketType.id)
            ? { ...item, quantity: item.quantity + quantity }
            : item
        );
      } else {
        newItems = [...prev, { ticketType, quantity, event }];
      }
      console.log('[Cart] New cart items:', newItems.length);
      return newItems;
    });
  };

  const removeFromCart = (ticketTypeId: string) => {
    console.log('[Cart] Removing from cart:', ticketTypeId);
    setItems(prev => prev.filter(item => !ticketTypeIdEquals(item.ticketType.id, ticketTypeId)));
  };

  const updateQuantity = (ticketTypeId: string, quantity: number) => {
    console.log('[Cart] Updating quantity:', ticketTypeId, quantity);
    if (quantity <= 0) {
      removeFromCart(ticketTypeId);
      return;
    }
    setItems(prev =>
      prev.map(item =>
        ticketTypeIdEquals(item.ticketType.id, ticketTypeId) ? { ...item, quantity } : item
      )
    );
  };

  const clearCart = () => {
    console.log('[Cart] Clearing cart');
    setItems([]);
  };

  const totalAmount = items.reduce(
    (sum, item) => sum + (item.ticketType.price || 0) * item.quantity,
    0
  );

  const totalItems = items.reduce((sum, item) => sum + item.quantity, 0);

  console.log('[Cart] totalItems:', totalItems);

  return (
    <CartContext.Provider
      value={{
        items,
        addToCart,
        removeFromCart,
        updateQuantity,
        clearCart,
        totalAmount,
        totalItems,
      }}
    >
      {children}
    </CartContext.Provider>
  );
}

export function useCart() {
  const context = useContext(CartContext);
  if (context === undefined) {
    throw new Error('useCart must be used within a CartProvider');
  }
  return context;
}
