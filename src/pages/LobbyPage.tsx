import { useState, useEffect, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { useAuth } from "../context/AuthContext";
import { getGames, createGame, joinGame } from "../api/games";
import GameList from "../components/GameList";
import type { GameSummary } from "../types";
import type { ApiError } from "../api/client";

export default function LobbyPage() {
  const { token, user, logout } = useAuth();
  const navigate = useNavigate();

  const [games, setGames] = useState<GameSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [joiningId, setJoiningId] = useState<string | null>(null);

  const fetchGames = useCallback(async () => {
    if (!token) return;
    setError(null);
    try {
      const list = await getGames(token, "waiting");
      setGames(list);
    } catch (err) {
      setError((err as ApiError).message || "対局一覧の取得に失敗しました");
    } finally {
      setIsLoading(false);
    }
  }, [token]);

 useEffect(() => {
  fetchGames();

  let interval: ReturnType<typeof setInterval> | null = null;

  function startPolling() {
    if (interval) return;
    interval = setInterval(fetchGames, 5000);
  }

  function stopPolling() {
    if (interval) {
      clearInterval(interval);
      interval = null;
    }
  }

  function handleVisibilityChange() {
    if (document.hidden) {
      stopPolling();
    } else {
      fetchGames();
      startPolling();
    }
  }

  startPolling();
  document.addEventListener("visibilitychange", handleVisibilityChange);

  return () => {
    stopPolling();
    document.removeEventListener("visibilitychange", handleVisibilityChange);
  };
}, [fetchGames]);
 
  async function handleCreate() {
    if (!token) return;
    setIsCreating(true);
    setError(null);
    try {
      const res = await createGame(token);
      navigate(`/games/${res.game_id}`);
    } catch (err) {
      setError((err as ApiError).message || "対局の作成に失敗しました");
      setIsCreating(false);
    }
  }

  async function handleJoin(gameId: string) {
    if (!token) return;
    setJoiningId(gameId);
    setError(null);
    try {
      await joinGame(gameId, token);
      navigate(`/games/${gameId}`);
    } catch (err) {
      setError((err as ApiError).message || "対局への参加に失敗しました");
      setJoiningId(null);
    }
  }

  return (
    <div className="lobby-page">
      <header className="lobby-header">
        <h1>ロビー</h1>
        <div>
          <span>{user?.username} さん</span>
          <button onClick={logout}>ログアウト</button>
        </div>
      </header>

      <button onClick={handleCreate} disabled={isCreating}>
        {isCreating ? "作成中..." : "新規対局を作成"}
      </button>

      {error && <p className="lobby-error">{error}</p>}

      {isLoading ? (
        <p>読み込み中...</p>
      ) : (
        user && (
          <GameList
            games={games}
            currentUserId={user.id}
            onJoin={handleJoin}
            joiningId={joiningId}
          />
        )
      )}
    </div>
  );
}