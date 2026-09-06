import { apiClient } from "./client";
import type {
  ClaimAbandonmentResponse,
  GameCreatedResponse,
  GameDetailResponse,
  GameStateResponse,
  GameSummary,
  JoinGameResponse,
  MoveRow,
  ResignGameResponse,
} from "../types";

export function createGame(token: string) {
  return apiClient.post<GameCreatedResponse>("/games", {}, token);
}

export function getGame(gameId: string) {
  return apiClient.get<GameDetailResponse>(`/games/${gameId}`);
}

export function joinGame(gameId: string, token: string) {
  return apiClient.post<JoinGameResponse>(`/games/${gameId}/join`, {}, token);
}

export function makeMove(gameId: string, uci: string, token: string) {
  return apiClient.post<GameStateResponse>(`/games/${gameId}/move`, { uci }, token);
}

export function resignGame(gameId: string, token: string) {
  return apiClient.post<ResignGameResponse>(`/games/${gameId}/resign`, {}, token);
}

export function getGames(token: string, status?: string) {
  const query = status ? `?status=${status}` : "";
  return apiClient.get<GameSummary[]>(`/games${query}`, token);
}

export function getMoves(id: string, token: string) {
  return apiClient.get<MoveRow[]>(`/games/${id}/moves`, token);
}

export function claimAbandonment(gameId: string, token: string) {
  return apiClient.post<ClaimAbandonmentResponse>(
    `/games/${gameId}/claim-abandonment`,
    {},
    token
  );
}