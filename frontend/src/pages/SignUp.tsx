import { useState, type FormEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Form, Input, Button } from '../components/core/Form';
import { useAuthStore } from '../stores/authStore';
import logoSvg from '../assets/logo.svg';

export default function SignUp() {
  const navigate = useNavigate();
  const { requestOtp, signup, isLoading, error, clearError } = useAuthStore();

  const [email, setEmail] = useState('');
  const [code, setCode] = useState('');
  const [step, setStep] = useState<'email' | 'otp'>('email');

  const handleRequestOtp = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    const success = await requestOtp(email);
    if (success) {
      setStep('otp');
    }
  };

  const handleSignup = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    const success = await signup(email, code);
    if (success) {
      navigate('/');
    }
  };

  return (
    <div className="min-h-screen bg-black flex flex-col items-center justify-center px-4">
      <Link to="/" className="mb-8">
        <img src={logoSvg} alt="mExchange" className="h-8" />
      </Link>

      {step === 'email' ? (
        <Form
          title="Create account"
          subtitle="Enter your email to get started"
          onSubmit={handleRequestOtp}
          footer={
            <>
              Already have an account?{' '}
              <Link to="/signin" className="text-white hover:underline">
                Sign in
              </Link>
            </>
          }
        >
          <Input
            label="Email"
            type="email"
            placeholder="you@example.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            error={error || undefined}
            required
          />
          <Button type="submit" loading={isLoading} className="mt-2">
            Continue
          </Button>
        </Form>
      ) : (
        <Form
          title="Verify your email"
          subtitle={`We sent a code to ${email}`}
          onSubmit={handleSignup}
          footer={
            <button
              type="button"
              onClick={() => setStep('email')}
              className="text-white hover:underline"
            >
              Use different email
            </button>
          }
        >
          <Input
            label="Verification code"
            type="text"
            placeholder="000000"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            error={error || undefined}
            maxLength={6}
            required
          />
          <Button type="submit" loading={isLoading} className="mt-2">
            Create account
          </Button>
        </Form>
      )}
    </div>
  );
}
