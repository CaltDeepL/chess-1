import type { CSSProperties } from "react";
import type { PieceRenderObject } from "react-chessboard";
import GlassPiece from "./GlassPiece";

/**
 * ガラス/クリスタル調 x ピクトグラムのチェス駒セット。
 * react-chessboard の customPieces prop にそのまま渡せる形式。
 */

const PIECE_TYPES = ["P", "N", "B", "R", "Q", "K"] as const;

export const glassPieces: PieceRenderObject = {};

for (const color of ["w", "b"] as const) {
  for (const type of PIECE_TYPES) {
    glassPieces[`${color}${type}`] = () => <GlassPiece type={type} color={color} />;
  }
}

export const glassBoardWrapperStyle: CSSProperties = {
  borderRadius: 16,
  padding: 12,
};
