import { useState, useEffect, useCallback } from "react";
import { useNavigate, Link } from "react-router-dom";
import { useAuth } from "../context/useAuth";
import { getRanking } from "../api/ranking";
import type { RankingEntry } from "../types";
import type { ApiError } from "../api/client";

export default function RankingPage() {
  const { token, user } = useAuth();
  const navigate = useNavigate();

  const [entries, setEntries] = useState<RankingEntry[]>([]);
  const [me, setMe] = useState<RankingEntry | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchRanking = useCallback(async () => {
    setError(null);
    try {
      const res = await getRanking(token, { limit: 50 });
      setEntries(res.entries);
      setMe(res.me);
    } catch (err) {
      setError((err as ApiError).message || "ランキングの取得に失敗しました");
    } finally {
      setIsLoading(false);
    }
  }, [token]);

  useEffect(() => {
    // 他ページと同様、effect本体から直接呼ばずマイクロタスク経由にする
    queueMicrotask(fetchRanking);
  }, [fetchRanking]);

  // 上位50件に自分が含まれていれば、下部に重ねて出さない
  const isMeListed = me !== null && entries.some((e) => e.user_id === me.user_id);

  return (
    <div className="ranking-page">
      <header className="ranking-header">
        <h1>ランキング</h1>
        <Link to={token ? "/lobby" : "/login"}>
          {token ? "ロビーへ戻る" : "ログイン"}
        </Link>
      </header>

      {error && <p className="ranking-error">{error}</p>}

      {isLoading ? (
        <p>読み込み中...</p>
      ) : entries.length === 0 ? (
        <p className="ranking-empty">
          まだ対局が行われていません。1局終えるとランキングに載ります。
        </p>
      ) : (
        <table className="ranking-table">
          <thead>
            <tr>
              <th scope="col">順位</th>
              <th scope="col">プレイヤー</th>
              <th scope="col">レーティング</th>
              <th scope="col">対局数</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry) => (
              <tr
                key={entry.user_id}
                className={entry.user_id === user?.id ? "is-me" : ""}
              >
                <td>{entry.rank}</td>
                <td>{entry.username}</td>
                <td>{entry.rating}</td>
                <td>{entry.games_played}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {me && !isMeListed && (
        <div className="ranking-me">
          <p>あなたの順位</p>
          <table className="ranking-table">
            <tbody>
              <tr className="is-me">
                <td>{me.rank}</td>
                <td>{me.username}</td>
                <td>{me.rating}</td>
                <td>{me.games_played}</td>
              </tr>
            </tbody>
          </table>
        </div>
      )}

      {token && (
        <div className="ranking-actions">
          <button onClick={() => navigate("/history")}>対局履歴を見る</button>
        </div>
      )}
    </div>
  );
}
