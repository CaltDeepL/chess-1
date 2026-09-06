import { useState, useEffect, useCallback } from "react";
import { useNavigate, Link } from "react-router-dom";
import { useAuth } from "../context/useAuth";
import { getMyGames } from "../api/history";
import type { GameHistoryItem } from "../types";
import type { ApiError } from "../api/client";

const PAGE_SIZE = 20;

const OUTCOME_LABELS: Record<string, string> = {
  win: "勝ち",
  loss: "負け",
  draw: "引き分け",
};

const END_REASON_LABELS: Record<string, string> = {
  checkmate: "チェックメイト",
  resignation: "投了",
  stalemate: "ステイルメイト",
  insufficient_material: "駒不足",
};

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString("ja-JP", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function HistoryPage() {
  const { token, user, logout } = useAuth();
  const navigate = useNavigate();

  const [games, setGames] = useState<GameHistoryItem[]>([]);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchPage = useCallback(
    async (nextOffset: number) => {
      if (!token) return;
      setError(null);
      setIsLoading(true);
      try {
        const list = await getMyGames(token, { limit: PAGE_SIZE, offset: nextOffset });
        setGames(list);
        setOffset(nextOffset);
        // 満杯で返ってきたら次のページがある可能性がある。
        // 総件数を返すAPIにしていないので、この判定で足りる範囲に留める。
        setHasMore(list.length === PAGE_SIZE);
      } catch (err) {
        setError((err as ApiError).message || "対局履歴の取得に失敗しました");
      } finally {
        setIsLoading(false);
      }
    },
    [token]
  );

  useEffect(() => {
    // ロビーと同じく、effect本体から直接呼ばずマイクロタスク経由にする
    // (react-hooks/set-state-in-effect を避けるため)
    queueMicrotask(() => fetchPage(0));
  }, [fetchPage]);

  function handleLogout() {
    logout();
    navigate("/login");
  }

  return (
    <div className="history-page">
      <header className="history-header">
        <h1>対局履歴</h1>
        <div>
          <span>{user?.username} さん</span>
          <button onClick={handleLogout}>ログアウト</button>
        </div>
      </header>

      <div className="history-actions">
        <Link to="/lobby">ロビーへ戻る</Link>
      </div>

      {error && <p className="history-error">{error}</p>}

      {isLoading ? (
        <p>読み込み中...</p>
      ) : games.length === 0 ? (
        <p className="history-empty">
          終了した対局はまだありません。<Link to="/lobby">ロビー</Link>から対局を始めてみてください。
        </p>
      ) : (
        <ul className="history-list">
          {games.map((game) => (
            <li key={game.game_id} className={`history-item history-item--${game.outcome ?? "unknown"}`}>
              <button
                className="history-item-button"
                onClick={() => navigate(`/games/${game.game_id}/review`)}
              >
                <span className="history-outcome">
                  {game.outcome ? OUTCOME_LABELS[game.outcome] : "―"}
                </span>
                <span className="history-opponent">
                  vs {game.opponent_username ?? "（相手なし）"}
                </span>
                <span className="history-color">
                  {game.my_color === "white" ? "白番" : "黒番"}
                </span>
                <span className="history-reason">
                  {game.end_reason
                    ? END_REASON_LABELS[game.end_reason] ?? game.end_reason
                    : "―"}
                </span>
                <span
                  className={`history-delta history-delta--${
                    game.my_rating_delta === null
                      ? "none"
                      : game.my_rating_delta >= 0
                      ? "up"
                      : "down"
                  }`}
                >
                  {game.my_rating_delta === null
                    ? ""
                    : `${game.my_rating_delta > 0 ? "+" : ""}${game.my_rating_delta}`}
                </span>
                <span className="history-moves">{game.move_count}手</span>
                <span className="history-date">{formatDate(game.finished_at)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="history-pagination">
        <button
          onClick={() => fetchPage(Math.max(0, offset - PAGE_SIZE))}
          disabled={offset === 0 || isLoading}
        >
          前へ
        </button>
        <button
          onClick={() => fetchPage(offset + PAGE_SIZE)}
          disabled={!hasMore || isLoading}
        >
          次へ
        </button>
      </div>
    </div>
  );
}
