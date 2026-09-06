import { useState } from "react";
import type { SyntheticEvent } from "react";
import { useNavigate, Link } from "react-router-dom";
import { login } from "../api/auth";
import { useAuth } from "../context/useAuth";
import type { ApiError } from "../api/client";
import PasswordInput from "../components/PasswordInput";

export default function LoginPage() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { login: setAuth } = useAuth();
  const navigate = useNavigate();

  async function handleSubmit(e: SyntheticEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    setIsSubmitting(true);

    try {
      const res = await login(username, password);
      // AuthResponseはuser_idのみでusernameを含まないため、
      // フォームに入力された値をそのまま表示名として使う
      setAuth(res.token, { id: res.user_id, username });
      navigate("/lobby", { replace: true });
    } catch (err) {
      const apiErr = err as ApiError;
      setError(
        apiErr.status === 401
          ? "ユーザー名またはパスワードが違います"
          : apiErr.message || "ログインに失敗しました"
      );
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <div className="auth-page">
      <form className="auth-form" onSubmit={handleSubmit}>
        <h1>ログイン</h1>

        <label htmlFor="username">ユーザー名</label>
        <input
          id="username"
          type="text"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          required
          autoComplete="username"
        />

        <PasswordInput
          value={password}
          onChange={setPassword}
          autoComplete="current-password"
          required
          disabled={isSubmitting}
        />

        {error && <p className="auth-error">{error}</p>}

        <button type="submit" disabled={isSubmitting}>
          {isSubmitting ? "ログイン中..." : "ログイン"}
        </button>

        <p className="auth-switch">
          アカウントをお持ちでない方は <Link to="/register">新規登録</Link>
        </p>
      </form>
    </div>
  );
}
