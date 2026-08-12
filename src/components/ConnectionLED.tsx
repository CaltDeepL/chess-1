type SocketStatus = "connecting" | "reconnecting" | "open" | "closed" | "error";

interface ConnectionLEDProps {
  status: SocketStatus;
}

const LABELS: Record<SocketStatus, string> = {
  connecting: "接続中",
  reconnecting: "再接続中",
  open: "接続済み",
  closed: "切断",
  error: "エラー",
};

export default function ConnectionLED({ status }: ConnectionLEDProps) {
  const colorClass =
    status === "open"
      ? "led-green"
      : status === "connecting" || status === "reconnecting"
      ? "led-yellow"
      : "led-red";

  return (
    <span className="led-wrapper" title={LABELS[status]}>
      <span className={`led ${colorClass}`} role="status" aria-label={LABELS[status]} />
    </span>
  );
}