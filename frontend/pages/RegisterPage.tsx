import { useState } from "react";
import type { SyntheticEvent } from "react";
import { useNavigate, Link } from "react-router-dom";
import { register } from "../api/auth";
import { useAuth } from "../context/useAuth";
import type { ApiError } from "../api/client";
import PasswordInput from "../components/PasswordInput";

export default function RegisterPage() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { login: setAuth } = useAuth();
  const navigate = useNavigate();

  async function handleSubmit(e: SyntheticEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);

    if (password !== confirmPassword) {
      setError("パスワードが一致しません");
      return;
    }

    setIsSubmitting(true);
    try {
      const res = await register(username, password);
      // loginと同様、AuthResponseはusernameを含まないため
      // フォーム入力値をそのまま表示名として使う
      setAuth(res.token, { id: res.user_id, username });
      navigate("/lobby", { replace: true });
    } catch (err) {
      const apiErr = err as ApiError;
      setError(
        apiErr.status === 409
          ? "このユーザー名はすでに使われています"
          : apiErr.message || "登録に失敗しました"
      );
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <div className="auth-page">
      <form className="auth-form" onSubmit={handleSubmit}>
        <h1>新規登録</h1>

        <label htmlFor="username">ユーザー名</label>
        <input
          id="username"
          type="text"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          required
        />

        <PasswordInput
          value={password}
          onChange={setPassword}
          autoComplete="new-password"
          hint="12文字以上。記号や数字は必須ではないので、覚えやすい文の組み合わせでも構いません"
          required
          disabled={isSubmitting}
        />

        <PasswordInput
          value={confirmPassword}
          onChange={setConfirmPassword}
          label="パスワード(確認)"
          autoComplete="new-password"
          required
          disabled={isSubmitting}
        />

        {error && <p className="auth-error">{error}</p>}

        <button type="submit" disabled={isSubmitting}>
          {isSubmitting ? "登録中..." : "登録する"}
        </button>

        <p className="auth-switch">
          すでにアカウントをお持ちの方は <Link to="/login">ログイン</Link>
        </p>
      </form>
    </div>
  );
}
