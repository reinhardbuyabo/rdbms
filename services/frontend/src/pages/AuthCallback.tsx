import { useEffect, useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { apiClient } from '@/lib/api';
import { toast } from 'sonner';

export default function AuthCallback() {
  const navigate = useNavigate();
  const location = useLocation();
  const [status, setStatus] = useState('Processing authentication...');

  useEffect(() => {
    const handleCallback = async () => {
      const hash = location.hash;
      const searchParams = new URLSearchParams(location.search);
      const token = searchParams.get('token') || hash.split('token=')[1]?.split('#')[0];
      const error = searchParams.get('error');

      if (error) {
        setStatus('Authentication failed. Redirecting...');
        toast.error('Authentication failed. Please try again.');
        setTimeout(() => navigate('/', { replace: true }), 1500);
        return;
      }

      if (!token) {
        setStatus('Invalid callback. Redirecting...');
        toast.error('Invalid authentication callback.');
        setTimeout(() => navigate('/', { replace: true }), 1500);
        return;
      }

      try {
        sessionStorage.setItem('auth_token', token);
        apiClient.setToken(token);
        setStatus('Authentication successful! Redirecting...');
        toast.success('Successfully signed in!');
        setTimeout(() => navigate('/events', { replace: true }), 500);
      } catch (err) {
        console.error('Auth callback error:', err);
        setStatus('Authentication failed. Redirecting...');
        toast.error('Failed to complete authentication.');
        setTimeout(() => navigate('/', { replace: true }), 1500);
      }
    };

    handleCallback();
  }, [location, navigate]);

  return (
    <div className="flex min-h-screen items-center justify-center">
      <div className="text-center">
        <div className="mx-auto mb-4 h-12 w-12 animate-spin rounded-full border-b-2 border-primary" />
        <p className="text-muted-foreground">{status}</p>
      </div>
    </div>
  );
}
