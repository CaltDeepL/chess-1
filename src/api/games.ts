import { apiClient } from "./client";
import type {
  GameCreatedResponse,
  GameStateResponse,
  JoinGameResponse,
  ResignGameResponse,
} from "../types";

export function createGame(token: string) {
  return apiClient.post<GameCreatedResponse>("/games", {}, token);
}

export function getGame(gameId: string) {
  return apiClient.get<GameStateResponse>(`/games/${gameId}`);
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
