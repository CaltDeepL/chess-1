import { useEffect, useState } from "react";
import type { GameEvent } from "../types";
import { wsUrl } from "../api/client";

export type ConnectionStatus = "connecting" | "open" | "reconnecting" | "closed" | "error";

interface UseGameSocketResult {
  status: ConnectionStatus;
  lastEvent: GameEvent | null;
}

const INITIAL_RECONNECT_DELAY_MS = 1000;
const MAX_RECONNECT_DELAY_MS = 30000;

export function useGameSocket(
  gameId: string | undefined,
  token: string | null
): UseGameSocketResult {
  const [status, setStatus] = useState<ConnectionStatus>("connecting");
  const [lastEvent, setLastEvent] = useState<GameEvent | null>(null);

  useEffect(() => {
    if (!gameId || !token) return;

    let socket: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let reconnectAttempt = 0;
    let stopped = false;

    function connect() {
      socket = new WebSocket(wsUrl(`/ws/games/${gameId}`));
      setStatus(reconnectAttempt === 0 ? "connecting" : "reconnecting");

      socket.onopen = () => {
        // 最初のメッセージでトークンを送って認証
        socket!.send(JSON.stringify({ token }));
        reconnectAttempt = 0;
        setStatus("open");
      };

      socket.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data) as GameEvent;
          setLastEvent(parsed);
        } catch {
          console.error("WebSocketメッセージのパースに失敗:", event.data);
        }
      };

      socket.onerror = () => {
        setStatus("error");
      };

      socket.onclose = () => {
        if (stopped) {
          setStatus("closed");
          return;
        }
        // 指数バックオフで再接続(1秒→2秒→4秒...最大30秒)
        const delay = Math.min(
          INITIAL_RECONNECT_DELAY_MS * 2 ** reconnectAttempt,
          MAX_RECONNECT_DELAY_MS
        );
        reconnectAttempt += 1;
        setStatus("reconnecting");
        reconnectTimer = setTimeout(connect, delay);
      };
    }

    connect();

    return () => {
      stopped = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      socket?.close();
    };
  }, [gameId, token]);

  return { status, lastEvent };
}
