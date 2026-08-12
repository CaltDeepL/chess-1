type SocketStatus = "connecting" | "reconnecting" | "open" | "closed" | "error";

interface ConnectionBannerProps {
  status: SocketStatus;
}

export default function ConnectionBanner({ status }: ConnectionBannerProps) {
  if (status === "open") return null;

  const message =
    status === "reconnecting"
      ? "接続が切れました。再接続しています..."
      : status === "connecting"
      ? "接続しています..."
      : "接続できません。ネットワークを確認してください。";

  return <div className="connection-banner">{message}</div>;
}
