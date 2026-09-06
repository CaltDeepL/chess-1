import { useState, useEffect } from "react";

interface Props {
  /** 切断が確定するまでの残り秒数。null なら相手は接続中 */
  initialRemaining: number | null;
  /** カウントが0になったときに呼ばれる */
  onExpire: () => void;
}

/**
 * 相手が切断したことを知らせ、勝ちが確定するまでの秒数を表示する。
 *
 * 残り秒数はサーバーから受け取った値を起点に、クライアント側で数える。
 * サーバーが時刻を送ると、両者の時計のずれがそのまま誤差になるため。
 */
export default function DisconnectCountdown({ initialRemaining, onExpire }: Props) {
  const [remaining, setRemaining] = useState<number | null>(initialRemaining);

  // 相手が再接続すると initialRemaining が null に戻る。
  // 他の画面と同様、effect本体から直接呼ばずマイクロタスク経由にする
  // (react-hooks/set-state-in-effect を避けるため)
  useEffect(() => {
    queueMicrotask(() => setRemaining(initialRemaining));
  }, [initialRemaining]);

  useEffect(() => {
    if (remaining === null) return;

    if (remaining <= 0) {
      // 判定はサーバーが行う。ここでは要求するだけなので、
      // クライアントの時計が進んでいても勝手に勝ちにはならない
      onExpire();
      return;
    }

    const timer = setTimeout(() => setRemaining((r) => (r === null ? null : r - 1)), 1000);
    return () => clearTimeout(timer);
  }, [remaining, onExpire]);

  if (remaining === null) return null;

  return (
    <div className="disconnect-banner" role="status" aria-live="polite">
      {remaining > 0 ? (
        <>
          <span className="disconnect-banner-title">対戦相手が接続を切りました</span>
          <span className="disconnect-banner-countdown">
            あと <strong>{remaining}</strong> 秒で戻らなければあなたの勝ちになります
          </span>
        </>
      ) : (
        <span className="disconnect-banner-title">勝ちを確定しています...</span>
      )}
    </div>
  );
}
