import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  ReactNode,
} from 'react';
import { User } from '@/types';
import { apiClient } from '@/lib/api';

interface AuthContextType {
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: () => void;
  logout: () => void;
  refreshUser: () => Promise<void>;
  mockLogin: (email: string) => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [initialLoad, setInitialLoad] = useState(true);

  const refreshUser = useCallback(async () => {
    setIsLoading(true);

    try {
      const token = apiClient.getToken();
      if (!token) {
        setUser(null);
        return;
      }

      const userData = await apiClient.getMe();
      setUser(userData);
    } catch (error) {
      console.error('Failed to fetch user:', error);
      apiClient.setToken(null);
      setUser(null);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const login = useCallback(() => {
    apiClient.loginWithGoogle();
  }, []);

  const mockLogin = useCallback(async (email: string) => {
    setIsLoading(true);
    try {
      await apiClient.mockLogin(email);
      const userData = await apiClient.getMe();
      setUser(userData);
    } catch (error) {
      console.error('Mock login failed:', error);
      apiClient.setToken(null);
      setUser(null);
      throw error;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const logout = useCallback(() => {
    apiClient.setToken(null);
    sessionStorage.removeItem('auth_token');
    setUser(null);
  }, []);

  useEffect(() => {
    let mounted = true;

    const loadUser = async () => {
      let token = apiClient.getToken();

      if (!token) {
        token = sessionStorage.getItem('auth_token');
        if (token) {
          apiClient.setToken(token);
        }
      }

      if (!token) {
        if (mounted) {
          setIsLoading(false);
          setInitialLoad(false);
        }
        return;
      }

      try {
        const userData = await apiClient.getMe();
        if (mounted) {
          setUser(userData);
        }
      } catch (error) {
        console.error('Failed to load user on init:', error);
        if (mounted) {
          apiClient.setToken(null);
          sessionStorage.removeItem('auth_token');
          setUser(null);
        }
      } finally {
        if (mounted) {
          setIsLoading(false);
          setInitialLoad(false);
        }
      }
    };

    loadUser();

    return () => {
      mounted = false;
    };
  }, []);

  const value: AuthContextType = {
    user,
    isAuthenticated: !!user,
    isLoading: isLoading || initialLoad,
    login,
    logout,
    refreshUser,
    mockLogin,
  };

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}
