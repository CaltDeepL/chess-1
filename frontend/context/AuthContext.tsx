import { useState, useEffect, useCallback } from "react";
import type { ReactNode } from "react";
import type { User } from "../types";
import { AuthContext } from "./auth-context";
import { logoutRequest } from "../api/auth";

const TOKEN_KEY = "chess_token";
const USER_KEY = "chess_user";

export function AuthProvider({ children }: { children: ReactNode }) {
  const [token, setToken] = useState<string | null>(() =>
    localStorage.getItem(TOKEN_KEY)
  );
  const [user, setUser] = useState<User | null>(() => {
    const raw = localStorage.getItem(USER_KEY);
    return raw ? (JSON.parse(raw) as User) : null;
  });

  useEffect(() => {
    if (token) {
      localStorage.setItem(TOKEN_KEY, token);
    } else {
      localStorage.removeItem(TOKEN_KEY);
    }
  }, [token]);

  useEffect(() => {
    if (user) {
      localStorage.setItem(USER_KEY, JSON.stringify(user));
    } else {
      localStorage.removeItem(USER_KEY);
    }
  }, [user]);

  const login = (newToken: string, newUser: User) => {
    setToken(newToken);
    setUser(newUser);
  };

  // 進行中の対局を終わらせる必要があるため、ローカルの状態を消すだけでは
  // 足りない。ただし通信の失敗でログアウトできなくなるのは困るので、
  // 失敗しても必ずローカルは消す。
  const logout = useCallback(async () => {
    if (token) {
      try {
        await logoutRequest(token);
      } catch {
        // 通信に失敗しても、ログアウト自体は完了させる。
        // 対局は切断扱いになり、猶予のあと自動で終了する
      }
    }
    setToken(null);
    setUser(null);
  }, [token]);

  return (
    <AuthContext.Provider
      value={{ token, user, login, logout, isAuthenticated: !!token }}
    >
      {children}
    </AuthContext.Provider>
  );
}