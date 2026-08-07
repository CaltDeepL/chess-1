import { useState, useEffect, useCallback } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useAuth } from "../context/AuthContext";
import { useGameSocket } from "../hooks/useGameSocket";
import { getGame, makeMove, resignGame } from "../api/games";
import ChessBoard from "../components/ChessBoard";
import type { GameDetailResponse, GameResult } from "../types";
import type { ApiError } from "../api/client";

export default function GamePage() {
  const { id } = useParams<{ id: string }>();
  const { token, user } = useAuth();
  const navigate = useNavigate();

  const [game, setGame] = useState<GameDetailResponse | null>(null);
  const [fen, setFen] = useState<string>("start");
  const [turn, setTurn] = useState<"white" | "black">("white");
  const [result, setResult] = useState<GameResult>(null);
  const [error, setError] = useState<string | null>(null);
  const [isResigning, setIsResigning] = useState(false);

  const { status, lastEvent } = useGameSocket(id, token);

  // 初回ロード: REST APIで現在の対局状態を取得
  useEffect(() => {
    if (!id || !token) return;
    getGame(id)
      .then((res) => {
        setGame(res);
        setFen(res.fen);
        setTurn(res.fen.split(" ")[1] === "b" ? "black" : "white");
        setResult(res.result);
      })
      .catch((err) => setError((err as ApiError).message || "対局情報の取得に失敗しました"));
  }, [id, token]);

  // WebSocketイベントで盤面を更新
  useEffect(() => {
    if (!lastEvent) return;

    if (lastEvent.type === "move") {
      setFen(lastEvent.fen);
      setTurn(lastEvent.fen.split(" ")[1] === "b" ? "black" : "white");
    } else if (lastEvent.type === "game_over") {
      setResult(lastEvent.result);
    }
  }, [lastEvent]);

  const myColor: "white" | "black" | null = !game || !user
    ? null
    : game.white_user_id === user.id
    ? "white"
    : game.black_user_id === user.id
    ? "black"
    : null;

  const isMyTurn = myColor !== null && myColor === turn && result === null;

  const handlePieceDrop = useCallback(
    (sourceSquare: string, targetSquare: string): boolean => {
      if (!id || !token || !isMyTurn) return false;

      const uci = `${sourceSquare}${targetSquare}`;
      const previousFen = fen;

      // 楽観的更新はせず、サーバーからの確定を待つ(WSイベントでfenが更新される)
      makeMove(id, uci, token).catch((err) => {
        setFen(previousFen); // 失敗時は盤面をそのまま維持(実質no-op、念のため)
        setError((err as ApiError).message || "その手は指せません");
      });

      // react-chessboardは同期的にtrue/falseを要求するため、
      // 実際の可否はサーバーレスポンス待ちだが、ここでは一旦trueを返し
      // 拒否された場合はサーバーの結果(WS再送 or エラー)で盤面を戻す設計にする
      return true;
    },
    [id, token, isMyTurn, fen]
  );

  async function handleResign() {
    if (!id || !token) return;
    setIsResigning(true);
    setError(null);
    try {
      await resignGame(id, token);
    } catch (err) {
      setError((err as ApiError).message || "投了に失敗しました");
    } finally {
      setIsResigning(false);
    }
  }

  if (!game) {
    return <div className="game-page">読み込み中...</div>;
  }

  return (
    <div className="game-page">
      <div className="game-status-bar">
        <span>接続状態: {status}</span>
        <span>手番: {turn === "white" ? "白" : "黒"}</span>
        {result && <span className="game-result">結果: {result}</span>}
      </div>

      <ChessBoard
        fen={fen}
        orientation={myColor ?? "white"}
        onPieceDrop={handlePieceDrop}
        isMyTurn={isMyTurn}
      />

      {error && <p className="game-error">{error}</p>}

      <div className="game-actions">
        <button onClick={handleResign} disabled={isResigning || result !== null}>
          {isResigning ? "投了中..." : "投了する"}
        </button>
        <button onClick={() => navigate("/lobby")}>ロビーへ戻る</button>
      </div>
    </div>
  );
}