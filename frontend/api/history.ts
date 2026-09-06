import { apiClient } from "./client";
import type { GameHistoryItem } from "../types";

/**
 * 自分が参加した終了済みの対局を新しい順に取得する。
 *
 * ロビーの getGames(token, "waiting") とは別のエンドポイントで、
 * 「他人の対局を探す」ではなく「自分の対局を見返す」ためのもの。
 */
export function getMyGames(
  token: string,
  options: { limit?: number; offset?: number } = {}
): Promise<GameHistoryItem[]> {
  const params = new URLSearchParams();
  if (options.limit !== undefined) params.set("limit", String(options.limit));
  if (options.offset !== undefined) params.set("offset", String(options.offset));

  const query = params.toString();
  return apiClient.get<GameHistoryItem[]>(
    `/users/me/games${query ? `?${query}` : ""}`,
    token
  );
}