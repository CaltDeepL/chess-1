import type { PieceRenderObject } from "react-chessboard";
import type { PieceType } from "../lib/pieceSymbols";
import UnicodePiece from "./UnicodePiece";

/**
 * Unicodeのチェス記号を使った駒表示。
 * SVGを描かずに文字だけで表現するため軽量。
 * react-chessboard の customPieces / pieces prop にそのまま渡せる形式。
 */

const PIECE_TYPES: PieceType[] = ["P", "N", "B", "R", "Q", "K"];

export const unicodeGlassPieces: PieceRenderObject = {};

for (const color of ["w", "b"] as const) {
  for (const type of PIECE_TYPES) {
    unicodeGlassPieces[`${color}${type}`] = () => <UnicodePiece type={type} color={color} />;
  }
}
