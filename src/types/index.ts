export interface User {
  id: string;
  username: string;
}

// バックエンドの /auth/register, /auth/login レスポンス(user_idのみ、usernameは含まない)
export interface AuthResponse {
  user_id: string;
  token: string;
}

export type GameStatus = "waiting" | "in_progress" | "finished";
export type GameResult = "white_win" | "black_win" | "draw" | null;

export interface Game {
  id: string;
  whiteUserId: string;
  blackUserId: string | null;
  status: GameStatus;
  result: GameResult;
  fen: string;
}

export interface MoveRequest {
  uci: string;
}

// POST /games のレスポンス
export interface GameCreatedResponse {
  game_id: string;
  fen: string;
}

// GET /games/:id, POST /games/:id/move のレスポンス
export interface GameStateResponse {
  game_id: string;
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
  | { type: "move"; fen: string; uci: string; is_check: boolean; is_game_over: boolean }
  | { type: "game_over"; result: GameResult; end_reason: string };
