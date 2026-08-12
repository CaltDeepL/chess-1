import type { PieceRenderObject } from "react-chessboard";

/**
 * Unicodeのチェス記号を使った駒表示。
 * SVGを描かずに文字だけで表現するため軽量。
 * react-chessboard の customPieces / pieces prop にそのまま渡せる形式。
 */

export type PieceColor = "w" | "b";
export type PieceType = "K" | "Q" | "R" | "B" | "N" | "P";

// Unicodeのチェス記号。白番と黒番で別のコードポイントが割り当てられている。
export const UNICODE_SYMBOLS: Record<PieceColor, Record<PieceType, string>> = {
  w: { K: "♔", Q: "♕", R: "♖", B: "♗", N: "♘", P: "♙" },
  b: { K: "♚", Q: "♛", R: "♜", B: "♝", N: "♞", P: "♟" },
};

function UnicodePiece({ type, color }: { type: PieceType; color: PieceColor }) {
  return (
    <span className={`glass-piece glass-piece--${color}`} aria-hidden="true">
      {UNICODE_SYMBOLS[color][type]}
    </span>
  );
}

const PIECE_TYPES: PieceType[] = ["P", "N", "B", "R", "Q", "K"];

export const unicodeGlassPieces: PieceRenderObject = {};

for (const color of ["w", "b"] as const) {
  for (const type of PIECE_TYPES) {
    unicodeGlassPieces[`${color}${type}`] = () => <UnicodePiece type={type} color={color} />;
  }
}
