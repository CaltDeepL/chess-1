import { useState, useEffect, useCallback } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useAuth } from "../context/useAuth";
import { useToast } from "../context/useToast";
import { useGameSocket } from "../hooks/useGameSocket";
import { claimAbandonment, getGame, getMoves, makeMove, resignGame } from "../api/games";
import { getUser } from "../api/users";
import ChessBoard from "../components/ChessBoard";
import { UNICODE_SYMBOLS } from "../lib/pieceSymbols";
import ConnectionLED from "../components/ConnectionLED";
import ConnectionBanner from "../components/ConnectionBanner";
import DisconnectCountdown from "../components/DisconnectCountdown";
import GameMenu from "../components/GameMenu";
import GameOverOverlay from "../components/GameOverOverlay";
import MoveHistory from "../components/MoveHistory";
import type { GameDetailResponse, GameEvent, GameResult, MoveRow } from "../types";
import type { ApiError } from "../api/client";

type PromotionPiece = "q" | "r" | "b" | "n";

const PROMOTION_CHOICES: { piece: PromotionPiece; label: "Q" | "R" | "B" | "N" }[] = [
  { piece: "q", label: "Q" },
  { piece: "r", label: "R" },
  { piece: "b", label: "B" },
  { piece: "n", label: "N" },
];

export default function GamePage() {
  const { id } = useParams<{ id: string }>();
  const { token, user, logout } = useAuth();
  const { showToast } = useToast();
  const navigate = useNavigate();

  const [game, setGame] = useState<GameDetailResponse | null>(null);
  const [fen, setFen] = useState<string>("start");
  const [turn, setTurn] = useState<"white" | "black">("white");
  const [result, setResult] = useState<GameResult>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isResigning, setIsResigning] = useState(false);
  const [opponentId, setOpponentId] = useState<string | null>(null);
  const [opponent, setOpponent] = useState<{ username: string; rating: number } | null>(
    null
  );
  const [pendingPromotion, setPendingPromotion] = useState<{
    source: string;
    target: string;
  } | null>(null);
  const [moves, setMoves] = useState<MoveRow[]>([]);
  const [opponentGrace, setOpponentGrace] = useState<number | null>(null);

  // WebSocketイベントを受信するたびに盤面を更新する(useGameSocket内のonmessageから直接呼ばれる)
  const handleGameEvent = useCallback(
    (event: GameEvent) => {
      if (event.type === "move") {
        setFen(event.fen);
        setTurn(event.fen.split(" ")[1] === "b" ? "black" : "white");
        setMoves((prev) => [
          ...prev,
          { move_number: prev.length + 1, uci: event.uci, fen_after: event.fen },
        ]);
      } else if (event.type === "opponent_joined") {
        setOpponentId(event.user_id);
        setGame((prev) =>
          prev ? { ...prev, black_user_id: event.user_id, status: "in_progress" } : prev
        );
        showToast("対戦相手が参加しました");
      } else if (event.type === "player_disconnected") {
        // 自分の切断イベントは無視する(自分は今この画面を見ている)
        if (event.user_id !== user?.id) {
          setOpponentGrace(event.remaining_seconds);
        }
      } else if (event.type === "player_reconnected") {
        if (event.user_id !== user?.id) {
          setOpponentGrace(null);
        }
      } else if (event.type === "game_over") {
        setResult(event.result);
        setOpponentGrace(null);
      }
    },
    [showToast, user?.id]
  );

  const status = useGameSocket(id, token, handleGameEvent);

  // 初回ロード: 棋譜(指し手履歴)を取得
  useEffect(() => {
    if (!id || !token) return;
    getMoves(id, token)
      .then(setMoves)
      .catch(() => {});
  }, [id, token]);

  // 初回ロード: REST APIで現在の対局状態を取得
  useEffect(() => {
    if (!id || !token) return;
    getGame(id)
      .then((res) => {
        setLoadError(null);
        setGame(res);
        setFen(res.fen);
        setTurn(res.fen.split(" ")[1] === "b" ? "black" : "white");
        setResult(res.result);

        if (user) {
          const opponent =
            res.white_user_id === user.id ? res.black_user_id : res.white_user_id;
          setOpponentId(opponent);
        }
      })
      .catch((err) => setLoadError((err as ApiError).message || "対局情報の取得に失敗しました"));
  }, [id, token, user]);

  // 対戦相手のIDが判明したら表示名を取得する
  useEffect(() => {
    if (!opponentId) return;
    getUser(opponentId)
      .then((res) => setOpponent({ username: res.username, rating: res.rating }))
      .catch(() => setOpponent(null));
  }, [opponentId]);

  const myColor: "white" | "black" | null = !game || !user
    ? null
    : game.white_user_id === user.id
    ? "white"
    : game.black_user_id === user.id
    ? "black"
    : null;

  // WS接続が確立していない間は指し手も投了も止める。
  // makeMove/resignGame自体はRESTなのでWS未接続でも通ってしまうが、その間は
  // サーバー側の確定イベント(WS)を受け取れず盤面が更新されないため、操作不能扱いにする。
  const isConnected = status === "open";
  const isMyTurn = myColor !== null && myColor === turn && result === null && isConnected;

  const handlePieceDrop = useCallback(
    (sourceSquare: string, targetSquare: string, promotion?: string): boolean => {
      if (!id || !token || !isMyTurn) return false;
      // プロモーション選択待ちの間は、それを確定させる呼び出し(promotion付き)以外は
      // 受け付けない。盤面のドラッグ自体は下のisMyTurn経由で既に止めているが、
      // ここでも二重に防ぎ、古いpendingPromotionの値が無関係な手と混ざるのを防ぐ。
      if (pendingPromotion && !promotion) return false;

      const uci = `${sourceSquare}${targetSquare}${promotion ?? ""}`;
      const previousFen = fen;

      // 楽観的更新はせず、サーバーからの確定を待つ(WSイベントでfenが更新される)
      makeMove(id, uci, token).catch((err) => {
        setFen(previousFen); // 失敗時は盤面をそのまま維持(実質no-op、念のため)
        showToast((err as ApiError).message || "その手は指せません");
      });

      // react-chessboardは同期的にtrue/falseを要求するため、
      // 実際の可否はサーバーレスポンス待ちだが、ここでは一旦trueを返し
      // 拒否された場合はサーバーの結果(WS再送 or エラー)で盤面を戻す設計にする
      return true;
    },
    [id, token, isMyTurn, fen, pendingPromotion, showToast]
  );

  const handlePromotionNeeded = useCallback((sourceSquare: string, targetSquare: string) => {
    // 既に選択待ちがある場合は上書きしない(通常は盤面側のドラッグ無効化で
    // 到達しないはずだが、念のための二重ガード)
    setPendingPromotion((current) => current ?? { source: sourceSquare, target: targetSquare });
  }, []);

  function handlePromotionSelect(piece: PromotionPiece) {
    if (!pendingPromotion) return;
    handlePieceDrop(pendingPromotion.source, pendingPromotion.target, piece);
    setPendingPromotion(null);
  }

  function handlePromotionCancel() {
    setPendingPromotion(null);
  }

  async function handleResign() {
    if (!id || !token || !isConnected) return;
    setIsResigning(true);
    try {
      await resignGame(id, token);
    } catch (err) {
      showToast((err as ApiError).message || "投了に失敗しました");
    } finally {
      setIsResigning(false);
    }
  }

  // カウントが0になったらサーバーに判定を要求する。
  // 判定はサーバーが行うので、クライアントの時計がずれていても
  // 猶予前に勝ちになることはない
  const handleGraceExpired = useCallback(() => {
    if (!id || !token) return;
    claimAbandonment(id, token).catch(() => {
      // 失敗してもsweepが拾う。画面は「確定しています」のまま
    });
  }, [id, token]);

  function handleLogout() {
    showToast("ログアウトしました");
    logout();
    navigate("/login");
  }

  if (!game) {
    return (
      <div className="game-page">
        {loadError ? (
          <>
            <p className="game-error">{loadError}</p>
            <div className="game-actions">
              <button onClick={() => navigate("/lobby")}>ロビーへ戻る</button>
            </div>
          </>
        ) : (
          "読み込み中..."
        )}
      </div>
    );
  }

  // 対局が進行中かどうか。opponent_joined で status を更新するようにしたので
  // 対局作成者側でも正しく true になる
  const isGameActive = result === null && game.status === "in_progress";

  return (
    <div className="game-page">
      <GameMenu
        onResign={handleResign}
        onLogout={handleLogout}
        resignDisabled={isResigning || result !== null || !isConnected}
        isGameActive={isGameActive}
      />

      <div className="game-status-bar">
        <ConnectionLED status={status} />
        <span>手番: {turn === "white" ? "白" : "黒"}</span>
        <span>
          対戦相手: {opponent ? `${opponent.username} (${opponent.rating})` : "参加待ち"}
        </span>
        {result && <span className="game-result">結果: {result}</span>}
      </div>

      <ConnectionBanner status={status} />

      <DisconnectCountdown initialRemaining={opponentGrace} onExpire={handleGraceExpired} />

      <div className="game-layout">
        <div className="game-board-area">
          <ChessBoard
            fen={fen}
            orientation={myColor ?? "white"}
            onPieceDrop={handlePieceDrop}
            onPromotionNeeded={handlePromotionNeeded}
            // プロモーション選択待ちの間は盤面のドラッグ自体を止める。
            // オーバーレイの見た目だけに頼ると、それを迂回する操作(自動化など)で
            // 選択待ちのまま別の手が成立してしまい、古いpendingPromotionの値と
            // 無関係な手が混ざる不具合につながる。
            isMyTurn={isMyTurn && !pendingPromotion}
          />

          {pendingPromotion && myColor && (
            <div className="promotion-picker-overlay" onClick={handlePromotionCancel}>
              <div className="promotion-picker" onClick={(e) => e.stopPropagation()}>
                <p>成る駒を選んでください</p>
                <div className="promotion-picker-choices">
                  {PROMOTION_CHOICES.map(({ piece, label }) => (
                    <button
                      key={piece}
                      className="promotion-piece-button"
                      onClick={() => handlePromotionSelect(piece)}
                      aria-label={label}
                    >
                      {UNICODE_SYMBOLS[myColor === "white" ? "w" : "b"][label]}
                    </button>
                  ))}
                </div>
                <button className="promotion-picker-cancel" onClick={handlePromotionCancel}>
                  キャンセル
                </button>
              </div>
            </div>
          )}
        </div>

        <MoveHistory moves={moves} />
      </div>

      {/* ロビーへ戻るも対局中は出さない。戻ってもその対局は進行中のままで、
          相手は指し手を待ち続ける */}
      {!isGameActive && (
        <div className="game-actions">
          <button onClick={() => navigate("/lobby")}>ロビーへ戻る</button>
        </div>
      )}

      {/* 対局中に抜ける手段が投了だけになるので、それが分かるようにしておく */}
      {isGameActive && (
        <p className="game-hint">対局中はロビーに戻れません。中断する場合は投了してください。</p>
      )}

      <GameOverOverlay result={result} myColor={myColor} />
    </div>
  );
}
