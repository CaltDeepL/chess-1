export interface User {
  id: string;
  username: string;
}

// GET /users/:id のレスポンス
export interface UserPublicResponse {
  id: string;
  username: string;
  rating: number;
}

// GET /users/ranking のレスポンス1件
export interface RankingEntry {
  rank: number;
  user_id: string;
  username: string;
  rating: number;
  games_played: number;
}

// GET /users/ranking のレスポンス
export interface RankingResponse {
  entries: RankingEntry[];
  /** 認証済みの場合のみ。上位圏外でも自分の順位が入る */
  me: RankingEntry | null;
}

// バックエンドの /auth/register, /auth/login レスポンス(user_idのみ、usernameは含まない)
export interface AuthResponse {
  user_id: string;
  token: string;
}

// POST /auth/logout のレスポンス
export interface LogoutResponse {
  /** 終了させた対局の数 */
  forfeited: number;
}

// POST /games/:id/claim-abandonment のレスポンス
export interface ClaimAbandonmentResponse {
  /** 対局を終了させたかどうか。false は「まだ猶予内」 */
  finished: boolean;
}

export type GameStatus = "waiting" | "in_progress" | "finished";
export type GameResult = "white_win" | "black_win" | "draw" | null;

export interface MoveRequest {
  uci: string;
}

// POST /games のレスポンス
export interface GameCreatedResponse {
  game_id: string;
  fen: string;
}

// POST /games/:id/move のレスポンス
export interface GameStateResponse {
  game_id: string;
  fen: string;
  is_check: boolean;
  is_game_over: boolean;
}

// GET /games/:id のレスポンス
export interface GameDetailResponse {
  game_id: string;
  white_user_id: string;
  black_user_id: string | null;
  status: GameStatus;
  result: GameResult;
  fen: string;
  is_check: boolean;
  is_game_over: boolean;
}

// POST /games/:id/join のレスポンス
export interface JoinGameResponse {
  game_id: string;
  status: GameStatus;
}

// POST /games/:id/resign のレスポンス
export interface ResignGameResponse {
  game_id: string;
  status: GameStatus;
  result: GameResult;
}

// WebSocketで受信するイベント(バックエンドのGameEventに対応、フィールド名はJSONそのまま)
export type GameEvent =
    { type: "connected" }
  | { type: "move"; fen: string; uci: string; is_check: boolean; is_game_over: boolean }
  | { type: "game_over"; result: GameResult; end_reason: string }
  | { type: "opponent_joined"; user_id: string }
  | { type: "player_disconnected"; user_id: string; remaining_seconds: number }
  | { type: "player_reconnected"; user_id: string };
   

export interface GameSummary {
  id: string;
  white_user_id: string;
  black_user_id: string | null;
  status: GameStatus;
  fen: string;
  created_at: string;
}


export interface MoveRow {
  move_number: number;
  uci: string;
  fen_after: string;
}

// GET /users/me/games のレスポンス1件
export interface GameHistoryItem {
  game_id: string;
  my_color: "white" | "black";
  opponent_username: string | null;
  result: GameResult;
  /** 自分視点の勝敗。判定できない場合は null */
  outcome: "win" | "loss" | "draw" | null;
  end_reason: string | null;
  move_count: number;
  finished_at: string;
  /** この対局でのレーティング変動。未適用なら null */
  my_rating_delta: number | null;
}