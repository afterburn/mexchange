import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuthStore } from '../stores/authStore';
import OrderHistory from '../components/OrderHistory';
import EmbedTopBar from '../components/EmbedTopBar';
import HeaderBar from '../components/HeaderBar';

export default function History() {
  const navigate = useNavigate();
  const { user } = useAuthStore();

  useEffect(() => {
    if (!user) {
      navigate('/signin');
    }
  }, [user, navigate]);

  if (!user) return null;

  return (
    <div className="min-h-screen bg-black text-white">
      <EmbedTopBar />
      <HeaderBar onLogout={() => navigate('/')} />

      <div className="max-w-4xl mx-auto px-4 py-8">
        <h1 className="text-2xl font-medium mb-6">Order History</h1>
        <OrderHistory pageSize={20} />
      </div>
    </div>
  );
}
