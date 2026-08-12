import { useEffect, useRef } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { useNavigate } from "react-router-dom";
import { AuthProvider, useAuth } from "./context/AuthContext";
import { ToastProvider, useToast } from "./context/ToastContext";
import { SESSION_EXPIRED_EVENT } from "./api/client";
import ErrorBoundary from "./components/ErrorBoundary";
import LoginPage from "./pages/LoginPage";
import RegisterPage from "./pages/RegisterPage";
import LobbyPage from "./pages/LobbyPage";
import GamePage from "./pages/GamePage";
import HomePage from "./pages/HomePage";

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuth();
  return isAuthenticated ? <>{children}</> : <Navigate to="/login" replace />;
}

// 認証済みAPIリクエストがどこかで401(トークン期限切れ/無効化)を受けたら、
// アプリのどのページからでも自動ログアウトしてログイン画面に戻す。
// client.ts側はReactの外なのでCustomEventで通知を受け取る。
function SessionExpiredListener() {
  const { logout, isAuthenticated } = useAuth();
  const { showToast } = useToast();
  const navigate = useNavigate();
  // StrictMode/複数タブ経由で401が近接して複数回飛んでくることがあるため、
  // isAuthenticatedのstate更新(非同期)を待たずに同期的に多重発火を防ぐ。
  const handledRef = useRef(false);

  useEffect(() => {
    if (isAuthenticated) handledRef.current = false;
  }, [isAuthenticated]);

  useEffect(() => {
    function handleSessionExpired() {
      if (handledRef.current || !isAuthenticated) return;
      handledRef.current = true;
      logout();
      showToast("セッションの有効期限が切れました。再度ログインしてください");
      navigate("/login", { replace: true });
    }

    window.addEventListener(SESSION_EXPIRED_EVENT, handleSessionExpired);
    return () => window.removeEventListener(SESSION_EXPIRED_EVENT, handleSessionExpired);
  }, [isAuthenticated, logout, showToast, navigate]);

  return null;
}

function AppRoutes() {
  return (
    <>
      <SessionExpiredListener />
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/register" element={<RegisterPage />} />
        <Route path="/" element={<HomePage />} />
        <Route
          path="/lobby"
          element={
            <ProtectedRoute>
              <LobbyPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/games/:id"
          element={
            <ProtectedRoute>
              <GamePage />
            </ProtectedRoute>
          }
        />
        <Route path="*" element={<Navigate to="/lobby" replace />} />
      </Routes>
    </>
  );
}

export default function App() {
  return (
    <ErrorBoundary>
      <AuthProvider>
        <ToastProvider>
          <BrowserRouter>
            <AppRoutes />
          </BrowserRouter>
        </ToastProvider>
      </AuthProvider>
    </ErrorBoundary>
  );
}
