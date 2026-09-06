import { apiClient } from "./client";
import type { UserPublicResponse } from "../types";

export function getUser(userId: string) {
  return apiClient.get<UserPublicResponse>(`/users/${userId}`);
}
