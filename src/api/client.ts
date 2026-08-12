// ビルド時に VITE_API_URL が設定されていればそれを使う(本番ビルド用)。
// 未設定時は開発時のAxumサーバーのアドレスにフォールバックする。
const BASE_URL = import.meta.env.VITE_API_URL ?? "http://localhost:3000";

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

// レスポンスがJSONエラーボディを返さなかった場合(バックエンド以外の
// プロキシ/ゲートウェイが割り込んだ場合など)のフォールバック文言。
const STATUS_FALLBACK_MESSAGES: Record<number, string> = {
  400: "リクエストの内容が正しくありません",
  401: "認証が必要です",
  403: "この操作を行う権限がありません",
  404: "見つかりませんでした",
  409: "競合が発生しました",
  429: "リクエストが多すぎます。しばらく待ってから再度お試しください",
  500: "サーバーエラーが発生しました",
  502: "サーバーに接続できません",
  503: "サーバーが混み合っています。しばらく待ってから再度お試しください",
};

// 認証済みリクエストがセッション切れ(401)になったとき、アプリ全体に
// 通知するためのイベント名。App.tsx側でリッスンし、自動ログアウト+
// ログイン画面への遷移を行う(ログイン/登録フォーム自体の401は
// tokenを送っていないため対象外 = 通常のパスワード誤り扱いのまま)。
const SESSION_EXPIRED_EVENT = "auth:session-expired";

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

  let res: Response;
  try {
    res = await fetch(`${BASE_URL}${path}`, { ...options, headers });
  } catch {
    // fetch自体が失敗 = ネットワーク断・サーバー未起動・CORSなど。
    // 素のTypeErrorのままだとstatusが無く呼び出し側のstatus判定が壊れるため、
    // ApiError形式に正規化する。
    const error: ApiError = {
      status: 0,
      message: "サーバーに接続できません。ネットワーク接続を確認してください",
    };
    throw error;
  }

  if (!res.ok) {
    let message = STATUS_FALLBACK_MESSAGES[res.status] ?? res.statusText;
    try {
      const body = await res.json();
      message = body.message ?? message;
    } catch {
      // JSONでないエラーレスポンスはそのまま
    }
    const error: ApiError = { status: res.status, message };

    if (res.status === 401 && token) {
      window.dispatchEvent(new CustomEvent(SESSION_EXPIRED_EVENT));
    }

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

export { wsUrl, SESSION_EXPIRED_EVENT };

export type { ApiError };