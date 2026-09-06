import { useState, useEffect, useMemo, useCallback } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useAuth } from "../context/useAuth";
import { getGame, getMoves } from "../api/games";
import ChessBoard from "../components/ChessBoard";
import { uciListToSan } from "../lib/uciToSan";
import type { GameDetailResponse, MoveRow } from "../types";
import type { ApiError } from "../api/client";

const START_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

export default function ReviewPage() {
  const { id } = useParams<{ id: string }>();
  const { token, user } = useAuth();
  const navigate = useNavigate();

  const [game, setGame] = useState<GameDetailResponse | null>(null);
  const [moves, setMoves] = useState<MoveRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  // -1 は初期局面。0 以降は moves[index] を指した直後の局面。
  const [index, setIndex] = useState(-1);

  useEffect(() => {
    if (!id || !token) return;
    getMoves(id, token)
      .then(setMoves)
      .catch(() => setMoves([]));
  }, [id, token]);

  useEffect(() => {
    if (!id) return;
    getGame(id)
      .then(setGame)
      .catch((err) =>
        setError((err as ApiError).message || "対局情報の取得に失敗しました")
      );
  }, [id]);

  // 盤面は fen_after をそのまま渡すだけでよい。
  // moves テーブルに各手の局面を保存してあるため(task-22)、
  // chess.js で初手から再現する必要がない。
  const fen = index < 0 ? START_FEN : moves[index]?.fen_after ?? START_FEN;

  const sans = useMemo(() => uciListToSan(moves.map((m) => m.uci)), [moves]);

  const myColor: "white" | "black" =
    game && user && game.black_user_id === user.id ? "black" : "white";

  const goTo = useCallback(
    (next: number) => {
      setIndex(Math.max(-1, Math.min(next, moves.length - 1)));
    },
    [moves.length]
  );

  // 矢印キーで手を送る。棋譜を見返すときはクリックより速い。
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "ArrowLeft") goTo(index - 1);
      else if (e.key === "ArrowRight") goTo(index + 1);
      else if (e.key === "Home") goTo(-1);
      else if (e.key === "End") goTo(moves.length - 1);
      else return;
      e.preventDefault();
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [index, moves.length, goTo]);

  if (error) {
    return (
      <div className="review-page">
        <p className="review-error">{error}</p>
        <button onClick={() => navigate("/history")}>履歴へ戻る</button>
      </div>
    );
  }

  return (
    <div className="review-page">
      <header className="review-header">
        <h1>棋譜の再生</h1>
        <button onClick={() => navigate("/history")}>履歴へ戻る</button>
      </header>

      <div className="review-status-bar">
        <span>
          {index < 0 ? "初期局面" : `${index + 1} / ${moves.length} 手目`}
        </span>
        {game?.result && <span className="review-result">結果: {game.result}</span>}
      </div>

      <div className="game-layout">
        <div className="game-board-area">
          {/* 読み取り専用。isMyTurn={false} でドラッグ自体が無効になる */}
          <ChessBoard
            fen={fen}
            orientation={myColor}
            onPieceDrop={() => false}
            onPromotionNeeded={() => {}}
            isMyTurn={false}
          />
        </div>

        <div className="review-moves">
          <ol className="review-move-list">
            <li>
              <button
                className={index < 0 ? "is-selected" : ""}
                onClick={() => goTo(-1)}
              >
                初期局面
              </button>
            </li>
            {sans.map((san, i) => (
              <li key={i}>
                <button
                  className={i === index ? "is-selected" : ""}
                  onClick={() => goTo(i)}
                >
                  <span className="review-move-number">
                    {i % 2 === 0 ? `${Math.floor(i / 2) + 1}.` : ""}
                  </span>
                  {san}
                </button>
              </li>
            ))}
          </ol>
        </div>
      </div>

      <div className="review-controls">
        <button onClick={() => goTo(-1)} disabled={index < 0} aria-label="最初へ">
          |◀
        </button>
        <button onClick={() => goTo(index - 1)} disabled={index < 0} aria-label="1手戻る">
          ◀
        </button>
        <button
          onClick={() => goTo(index + 1)}
          disabled={index >= moves.length - 1}
          aria-label="1手進む"
        >
          ▶
        </button>
        <button
          onClick={() => goTo(moves.length - 1)}
          disabled={index >= moves.length - 1}
          aria-label="最後へ"
        >
          ▶|
        </button>
      </div>

      <p className="review-hint">← → キーでも移動できます</p>
    </div>
  );
}
