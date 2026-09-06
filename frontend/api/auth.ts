import { apiClient } from "./client";
import type { AuthResponse } from "../types";

export function register(username: string, password: string) {
  return apiClient.post<AuthResponse>("/auth/register", { username, password });
}

export function login(username: string, password: string) {
  return apiClient.post<AuthResponse>("/auth/login", { username, password });
}