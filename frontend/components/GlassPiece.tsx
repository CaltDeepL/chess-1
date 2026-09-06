export type PieceColor = "w" | "b";

// ピクトグラム調(単純化された塗りつぶしシルエット)の駒形状。
// 各駒が一目で見分けられることを優先し、装飾的な彫刻表現は排除。
const SHAPES: Record<string, string> = {
  // ポーン: 丸い頭 + 台形の胴 + 台座
  P: "M50 24a10 10 0 110 20 10 10 0 010-20z M38 50h24l6 22H32z M28 76h44l4 10H24z",

  // ルーク: 城壁の凹凸 + 胴 + 台座
  R: "M30 22h9v9h-9zM45.5 22h9v9h-9zM61 22h9v9h-9z M30 31h40v8l-5 5v24l5 5v6H30v-6l5-5V44l-5-5z M26 79h48l4 9H22z",

  // ナイト: 馬の頭のシルエット(横向き)
  N: "M66 20c-16 0-26 12-26 24 0 6 2 10 6 14l-14 4-4 12h48l3-10c3-4 3-10 1-15-2-6 1-9 4-13 3-4 2-10-4-14a14 14 0 00-14-2z M60 28a3 3 0 11-6 0 3 3 0 016 0z M22 79h56l4 9H18z",

  // ビショップ: 先端の切れ込みが入った涙型
  B: "M50 16a7 7 0 017 7c0 3-2 5-4 7 6 5 10 13 10 21 0 9-6 16-13 16s-13-7-13-16c0-8 4-16 10-21-2-2-4-4-4-7a7 7 0 017-7z M46 32l8 8 8-8-8-6z M30 76h40l4 9H26z",

  // クイーン: 王冠に丸い突起5つ
  Q: "M26 34l5 8 6-16 6 12 7-14 7 14 6-12 6 16 5-8 3 32H23z M22 70h56l4 9H18z M28 83h44l3 7H25z",

  // キング: 十字 + 王冠の胴
  K: "M46 12h8v8h8v8h-8v5c11 4 19 15 19 27 0 9-7 16-16 16h-6c-9 0-16-7-16-16 0-12 8-23 19-27v-5h-8v-8h8z M22 70h56l4 9H18z",
};

const GLASS_FILL: Record<PieceColor, { top: string; bottom: string; stroke: string }> = {
  w: {
    top: "rgba(240, 248, 255, 0.95)",
    bottom: "rgba(175, 210, 240, 0.4)",
    stroke: "rgba(110, 155, 200, 0.95)",
  },
  b: {
    top: "rgba(70, 85, 105, 0.95)",
    bottom: "rgba(18, 25, 38, 0.6)",
    stroke: "rgba(10, 14, 22, 0.95)",
  },
};

export default function GlassPiece({ type, color }: { type: keyof typeof SHAPES; color: PieceColor }) {
  const g = GLASS_FILL[color];
  const gradId = `glass-grad-${type}-${color}`;
  const shineId = `glass-shine-${type}-${color}`;
  const clipId = `glass-clip-${type}-${color}`;
  const shadowId = `glass-shadow-${type}-${color}`;

  return (
    <svg viewBox="0 0 100 100" width="100%" height="100%" style={{ display: "block" }}>
      <defs>
        {/* 駒本体のガラス質感グラデーション */}
        <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={g.top} />
          <stop offset="100%" stopColor={g.bottom} />
        </linearGradient>

        {/* ハイライト用の斜め光沢(駒のシルエットでクリップするので形がはみ出さない) */}
        <linearGradient id={shineId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="rgba(255,255,255,0.9)" />
          <stop offset="35%" stopColor="rgba(255,255,255,0)" />
          <stop offset="100%" stopColor="rgba(255,255,255,0)" />
        </linearGradient>

        <clipPath id={clipId}>
          <path d={SHAPES[type]} />
        </clipPath>

        <filter id={shadowId} x="-30%" y="-30%" width="160%" height="160%">
          <feDropShadow dx="0" dy="2" stdDeviation="2" floodColor="rgba(0,0,0,0.35)" />
        </filter>
      </defs>

      <path
        d={SHAPES[type]}
        fill={`url(#${gradId})`}
        stroke={g.stroke}
        strokeWidth={1.5}
        filter={`url(#${shadowId})`}
      />

      {/* シルエットにクリップされた光沢帯。駒の形からはみ出さない */}
      <g clipPath={`url(#${clipId})`}>
        <rect x={0} y={0} width={100} height={100} fill={`url(#${shineId})`} />
      </g>
    </svg>
  );
}
