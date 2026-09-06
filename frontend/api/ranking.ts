import { apiClient } from "./client";
import type { RankingResponse } from "../types";

/**
 * レーティング順の一覧を取得する。
 *
 * 認証は必須ではないが、トークンを渡すとレスポンスに自分の順位（me）が
 * 含まれる。圏外でも自分の位置が分かるようにするため。
 */
export function getRanking(
  token?: string | null,
  options: { limit?: number } = {}
): Promise<RankingResponse> {
  const params = new URLSearchParams();
  if (options.limit !== undefined) params.set("limit", String(options.limit));

  const query = params.toString();
  return apiClient.get<RankingResponse>(
    `/users/ranking${query ? `?${query}` : ""}`,
    token
  );
}