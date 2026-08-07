const BASE_URL = "http://localhost:3000"; // 開発時のAxumサーバーのアドレス

// BASE_URLをWebSocket用のスキーム(ws/wss)に変換した値。
// 本番デプロイ時にBASE_URLだけ変更すればHTTP/WSの両方に反映される。
const WS_BASE_URL = BASE_URL.replace(/^http/, "ws");

function wsUrl(path: string): string {
  return `${WS_BASE_URL}${path}`;
}

interface ApiError {
  status: number;
  message: string;
}

async function request<T>(
  path: string,
  options: RequestInit = {},
  token?: string | null
): Promise<T> {
  const headers: HeadersInit = {
    "Content-Type": "application/json",
    ...(options.headers || {}),
  };

  if (token) {
    (headers as Record<string, string>)["Authorization"] = `Bearer ${token}`;
  }

  const res = await fetch(`${BASE_URL}${path}`, { ...options, headers });

  if (!res.ok) {
    let message = res.statusText;
    try {
      const body = await res.json();
      message = body.message ?? message;
    } catch {
      // JSONでないエラーレスポンスはそのまま
    }
    const error: ApiError = { status: res.status, message };
    throw error;
  }

  // 204 No Content等、bodyがない場合に対応
  const text = await res.text();
  return text ? (JSON.parse(text) as T) : (undefined as T);
}

export const apiClient = {
  get: <T>(path: string, token?: string | null) =>
    request<T>(path, { method: "GET" }, token),
  post: <T>(path: string, body: unknown, token?: string | null) =>
    request<T>(path, { method: "POST", body: JSON.stringify(body) }, token),
};

export { wsUrl };

export type { ApiError };