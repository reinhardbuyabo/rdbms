import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Layout } from '@/components/layout/Layout';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useCart } from '@/contexts/CartContext';
import { useAuth } from '@/contexts/AuthContext';
import { toast } from 'sonner';
import { CreditCard, Lock, CheckCircle, AlertCircle } from 'lucide-react';

function luhnCheck(cardNumber: string): boolean {
  const digits = cardNumber.replace(/\D/g, '');
  if (digits.length < 13 || digits.length > 19) return false;

  let sum = 0;
  let isEven = false;

  for (let i = digits.length - 1; i >= 0; i--) {
    let digit = parseInt(digits[i], 10);

    if (isEven) {
      digit *= 2;
      if (digit > 9) {
        digit -= 9;
      }
    }

    sum += digit;
    isEven = !isEven;
  }

  return sum % 10 === 0;
}

function getCardType(cardNumber: string): { name: string; pattern: RegExp; icon: string } {
  const digits = cardNumber.replace(/\D/g, '');

  if (/^4/.test(digits)) {
    return { name: 'Visa', pattern: /^4/, icon: '💳' };
  }
  if (/^5[1-5]/.test(digits) || /^2[2-7]/.test(digits)) {
    return { name: 'Mastercard', pattern: /^(5[1-5]|2[2-7])/, icon: '💳' };
  }
  if (/^3[47]/.test(digits)) {
    return { name: 'American Express', pattern: /^3[47]/, icon: '💳' };
  }
  if (/^6(?:011|5)/.test(digits)) {
    return { name: 'Discover', pattern: /^6(?:011|5)/, icon: '💳' };
  }
  return { name: 'Unknown', pattern: /./, icon: '💳' };
}

function formatCardNumber(value: string): string {
  const digits = value.replace(/\D/g, '');
  const groups = digits.match(/.{1,4}/g) || [];
  return groups.join(' ').substr(0, 19);
}

function formatExpiry(value: string): string {
  const digits = value.replace(/\D/g, '');
  if (digits.length >= 2) {
    return digits.substr(0, 2) + '/' + digits.substr(2, 2);
  }
  return digits;
}

const CheckoutPage = () => {
  const { items, totalAmount, clearCart } = useCart();
  const { user, isAuthenticated, login } = useAuth();
  const navigate = useNavigate();
  const [isProcessing, setIsProcessing] = useState(false);

  const [cardNumber, setCardNumber] = useState('');
  const [expiry, setExpiry] = useState('');
  const [cvc, setCvc] = useState('');
  const [cardName, setCardName] = useState('');
  const [cardNumberError, setCardNumberError] = useState('');
  const [expiryError, setExpiryError] = useState('');
  const [cvcError, setCvcError] = useState('');

  const serviceFee = totalAmount * 0.05;
  const grandTotal = totalAmount + serviceFee;

  const cardType = getCardType(cardNumber);
  const isCardValid = cardNumber.length >= 13 && luhnCheck(cardNumber);

  useEffect(() => {
    if (cardNumber.length > 0) {
      const digits = cardNumber.replace(/\D/g, '');
      if (digits.length < 13) {
        setCardNumberError('Card number is too short');
      } else if (!luhnCheck(digits)) {
        setCardNumberError('Invalid card number');
      } else {
        setCardNumberError('');
      }
    } else {
      setCardNumberError('');
    }
  }, [cardNumber]);

  useEffect(() => {
    if (expiry.length > 0) {
      const [month, year] = expiry.split('/');
      const currentDate = new Date();
      const currentYear = currentDate.getFullYear() % 100;
      const currentMonth = currentDate.getMonth() + 1;

      if (!month || !year) {
        setExpiryError('Invalid expiry date');
      } else if (parseInt(month) < 1 || parseInt(month) > 12) {
        setExpiryError('Invalid month');
      } else if (
        parseInt(year) < currentYear ||
        (parseInt(year) === currentYear && parseInt(month) < currentMonth)
      ) {
        setExpiryError('Card has expired');
      } else {
        setExpiryError('');
      }
    } else {
      setExpiryError('');
    }
  }, [expiry]);

  useEffect(() => {
    if (cvc.length > 0) {
      if (cvc.length < 3 || cvc.length > 4) {
        setCvcError('CVC must be 3-4 digits');
      } else {
        setCvcError('');
      }
    } else {
      setCvcError('');
    }
  }, [cvc]);

  const handleCheckout = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!isAuthenticated) {
      login('CUSTOMER');
      toast.info('Signed in! Proceeding with checkout...');
    }

    const digits = cardNumber.replace(/\D/g, '');
    if (!luhnCheck(digits)) {
      toast.error('Please enter a valid card number');
      return;
    }

    if (expiryError || cvcError) {
      toast.error('Please fix the errors in the form');
      return;
    }

    setIsProcessing(true);

    await new Promise(resolve => setTimeout(resolve, 2000));

    clearCart();
    setIsProcessing(false);
    toast.success('Payment successful! Your tickets are ready.');
    navigate('/my-tickets');
  };

  if (items.length === 0) {
    navigate('/cart');
    return null;
  }

  return (
    <Layout showFooter={false}>
      <div className="container max-w-4xl py-8 lg:py-12">
        <div className="mb-8">
          <h1 className="mb-2 text-3xl font-bold">Checkout</h1>
          <p className="text-muted-foreground">Complete your purchase securely</p>
        </div>

        <form onSubmit={handleCheckout}>
          <div className="grid gap-8 lg:grid-cols-5">
            {/* Payment Form */}
            <div className="space-y-6 lg:col-span-3">
              {/* Contact Info */}
              <Card>
                <CardHeader>
                  <CardTitle className="text-lg">Contact Information</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="grid gap-4 sm:grid-cols-2">
                    <div className="space-y-2">
                      <Label htmlFor="firstName">First Name</Label>
                      <Input
                        id="firstName"
                        defaultValue={user?.name?.split(' ')[0] || ''}
                        required
                      />
                    </div>
                    <div className="space-y-2">
                      <Label htmlFor="lastName">Last Name</Label>
                      <Input
                        id="lastName"
                        defaultValue={user?.name?.split(' ')[1] || ''}
                        required
                      />
                    </div>
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="email">Email</Label>
                    <Input id="email" type="email" defaultValue={user?.email || ''} required />
                  </div>
                </CardContent>
              </Card>

              {/* Payment Method */}
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2 text-lg">
                    <CreditCard className="h-5 w-5" />
                    Payment Method
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="space-y-2">
                    <Label htmlFor="cardNumber">Card Number</Label>
                    <div className="relative">
                      <Input
                        id="cardNumber"
                        placeholder="1234 5678 9012 3456"
                        value={cardNumber}
                        onChange={e => setCardNumber(formatCardNumber(e.target.value))}
                        maxLength={19}
                        className={
                          cardNumberError
                            ? 'border-red-500 pr-10'
                            : isCardValid
                              ? 'border-green-500 pr-10'
                              : ''
                        }
                        required
                      />
                      <div className="absolute right-3 top-1/2 -translate-y-1/2">
                        {cardNumber.length > 0 &&
                          (isCardValid ? (
                            <CheckCircle className="h-5 w-5 text-green-500" />
                          ) : (
                            <AlertCircle className="h-5 w-5 text-red-500" />
                          ))}
                      </div>
                    </div>
                    {cardNumber.length > 0 && (
                      <p className={`text-xs ${isCardValid ? 'text-green-600' : 'text-red-500'}`}>
                        {isCardValid ? `${cardType.name} ✓` : cardNumberError}
                      </p>
                    )}
                  </div>
                  <div className="grid grid-cols-2 gap-4">
                    <div className="space-y-2">
                      <Label htmlFor="expiry">Expiry Date</Label>
                      <Input
                        id="expiry"
                        placeholder="MM/YY"
                        value={expiry}
                        onChange={e => setExpiry(formatExpiry(e.target.value))}
                        maxLength={5}
                        className={
                          expiryError
                            ? 'border-red-500'
                            : expiry.length === 5 && !expiryError
                              ? 'border-green-500'
                              : ''
                        }
                        required
                      />
                      {expiryError && <p className="text-xs text-red-500">{expiryError}</p>}
                    </div>
                    <div className="space-y-2">
                      <Label htmlFor="cvc">CVC</Label>
                      <Input
                        id="cvc"
                        placeholder="123"
                        value={cvc}
                        onChange={e => setCvc(e.target.value.replace(/\D/g, '').substr(0, 4))}
                        maxLength={4}
                        className={
                          cvcError
                            ? 'border-red-500'
                            : cvc.length >= 3 && !cvcError
                              ? 'border-green-500'
                              : ''
                        }
                        required
                      />
                      {cvcError && <p className="text-xs text-red-500">{cvcError}</p>}
                    </div>
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="cardName">Name on Card</Label>
                    <Input
                      id="cardName"
                      value={cardName}
                      onChange={e => setCardName(e.target.value)}
                      placeholder="John Doe"
                      required
                    />
                  </div>
                </CardContent>
              </Card>
            </div>

            {/* Order Summary */}
            <div className="lg:col-span-2">
              <Card className="sticky top-24">
                <CardHeader>
                  <CardTitle className="text-lg">Order Summary</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  {items.map(item => (
                    <div key={String(item.ticketType.id)} className="flex justify-between text-sm">
                      <div>
                        <p className="font-medium">{item.event.title}</p>
                        <p className="text-muted-foreground">
                          {item.ticketType.name} x {item.quantity}
                        </p>
                      </div>
                      <p className="font-medium">
                        ${((item.ticketType.price || 0) * item.quantity).toFixed(2)}
                      </p>
                    </div>
                  ))}

                  <div className="space-y-2 border-t pt-4 text-sm">
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Subtotal</span>
                      <span>${totalAmount.toFixed(2)}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Service Fee</span>
                      <span>${serviceFee.toFixed(2)}</span>
                    </div>
                    <div className="flex justify-between border-t pt-2 text-lg font-bold">
                      <span>Total</span>
                      <span>${grandTotal.toFixed(2)}</span>
                    </div>
                  </div>

                  <Button
                    type="submit"
                    className="w-full"
                    size="lg"
                    disabled={
                      isProcessing || !isCardValid || !!expiryError || !!cvcError || !cardName
                    }
                  >
                    {isProcessing ? (
                      <>Processing...</>
                    ) : (
                      <>
                        <Lock className="mr-2 h-4 w-4" />
                        Pay ${grandTotal.toFixed(2)}
                      </>
                    )}
                  </Button>

                  <div className="flex items-center justify-center gap-2 text-xs text-muted-foreground">
                    <Lock className="h-3 w-3" />
                    Secure 256-bit SSL encryption
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        </form>
      </div>
    </Layout>
  );
};

export default CheckoutPage;
